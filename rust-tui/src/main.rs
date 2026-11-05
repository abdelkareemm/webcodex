//! WebCodex TUI. Inspired by the ratatui-based OpenAI Codex terminal frontend.
//! WebCodex's existing Python browser transport remains the execution engine.
use std::{env, io::{self, BufRead, BufReader, Write}, net::{TcpStream, SocketAddr}, sync::mpsc::{self, Receiver}, thread, time::Duration};
use crossterm::{event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind}, execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{backend::CrosstermBackend, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span, Text}, widgets::{Block, Borders, Clear, Paragraph, Wrap, Scrollbar, ScrollbarOrientation, ScrollbarState}, Terminal};
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Deserialize)]
struct Incoming { kind: String, #[serde(default)] title: String, #[serde(default)] body: String }
#[derive(Serialize)]
struct Outgoing<'a> { kind: &'a str, body: &'a str }
#[derive(Clone)]
struct Entry { kind: String, text: String }
struct App { cached_rows: Vec<Line<'static>>, cache_width: usize, transcript_dirty: bool, entries: Vec<Entry>, draft: String, cursor: usize, scroll: u16, tail: bool, busy: bool, approval: Option<String>, exit: bool, project: String, history: Vec<String>, history_index: Option<usize>, history_stash: String, status: String, ready: bool, max_scroll: u16, tick: u64 }
impl App {
    fn new(project: String) -> Self { Self { cached_rows: Vec::new(), cache_width: 0, transcript_dirty: true, entries: vec![], draft: String::new(), cursor: 0, scroll: 0, tail: true, busy: false, approval: None, exit: false, project, history: Vec::new(), history_index: None, history_stash: String::new(), status: "Starting WebCodex…".into(), ready: false, max_scroll: 0, tick: 0 } }
    fn push(&mut self, kind: String, text: String) { if kind == "clear" {self.entries.clear(); self.scroll=0; self.tail=true; self.transcript_dirty=true; return} let text=if kind=="assistant" {sanitize_display(&hide_code_blocks(&text))} else {sanitize_display(&text)}; if text.is_empty() && kind=="assistant" {return;} if kind=="user" && self.history.last()!=Some(&text) {self.history.push(text.clone());}
        self.entries.push(Entry{kind,text}); self.transcript_dirty=true; if self.tail { self.scroll=u16::MAX; } }
    fn stream(&mut self, text: String) {
        let text=sanitize_display(&hide_code_blocks(&text));
        if let Some(last)=self.entries.last_mut(){
            if last.kind=="stream" {last.text=text;self.transcript_dirty=true;return;}
        }
        self.push("stream".into(),text);
    }
    fn end_stream(&mut self, text: String) {
        let text=sanitize_display(&hide_code_blocks(&text));
        if let Some(last)=self.entries.last_mut(){
            if last.kind=="stream" {last.kind="assistant".into();last.text=text;self.transcript_dirty=true;return;}
        }
        if !text.is_empty(){self.push("assistant".into(),text);}
    }
    fn load_history(&mut self, messages: &[(String,String)]) {
        for (kind, text) in messages {
            self.push(kind.clone(), text.clone());
        }
        self.transcript_dirty = true;
    }
    fn move_scroll(&mut self, by: i32) {
        self.scroll=(self.scroll as i32 + by).clamp(0,self.max_scroll as i32) as u16;
        self.tail=self.scroll>=self.max_scroll;
    }
    fn jump_bottom(&mut self) {self.tail=true;self.scroll=self.max_scroll;}
    fn set_draft(&mut self, text:String) {self.draft=text;self.cursor=self.draft.len();}
    fn recall_prev(&mut self) {
        if self.history.is_empty(){return;}
        let next=match self.history_index {None=>{self.history_stash=self.draft.clone();self.history.len()-1},Some(i)=>i.saturating_sub(1)};
        self.history_index=Some(next);self.set_draft(self.history[next].clone());
    }
    fn recall_next(&mut self) {
        if let Some(i)=self.history_index {
            if i+1<self.history.len() {self.history_index=Some(i+1);self.set_draft(self.history[i+1].clone());}
            else {self.history_index=None;self.set_draft(self.history_stash.clone());}
        }
    }
    fn submit(&mut self,task:String) {
        self.history_index=None;self.history_stash.clear();
        self.push("user".into(),task);self.draft.clear();self.cursor=0;self.busy=true;self.jump_bottom();
    }
    fn put(&mut self, c:char) { self.draft.insert(self.cursor,c); self.cursor+=c.len_utf8(); }
    fn backspace(&mut self) { if self.cursor>0 {let i=self.draft[..self.cursor].char_indices().last().map(|(i,_)|i).unwrap_or(0); self.draft.drain(i..self.cursor); self.cursor=i;} }
    fn delete_forward(&mut self) { if self.cursor<self.draft.len() { let l=self.draft[self.cursor..].chars().next().unwrap().len_utf8();self.draft.drain(self.cursor..self.cursor+l); } }
}

// Never interpret control sequences from model or tool output as terminal
// commands. Escapes can alter cursor position and corrupt the ratatui buffer.
fn sanitize_display(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek().copied() {
                Some('[') => { // CSI
                    chars.next();
                    for c in chars.by_ref() { if ('@'..='~').contains(&c) { break; } }
                }
                Some(']') => { // OSC (BEL or ST terminator)
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' { break; }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') { chars.next(); break; }
                    }
                }
                Some(_) => { chars.next(); }
                None => {}
            }
            continue;
        }
        if ch == '\r' { result.push('\n'); }
        else if ch == '\n' || ch == '\t' || !ch.is_control() { result.push(ch); }
    }
    result
}

// Defense in depth: hide fenced model code/edit blocks in every assistant
// message, including partial streaming updates and restored history.
fn hide_code_blocks(s:&str)->String {
    let mut result=Vec::new();
    let mut fence:Option<(char,usize)>=None;
    let mut edit_block=false;
    let mut html_pre=false;
    for line in s.lines() {
        let t=line.trim_start();
        let marker=t.chars().next().unwrap_or(' ');
        let n=if marker=='`'||marker=='~' {t.chars().take_while(|&c|c==marker).count()}else{0};
        if let Some((c,len))=fence {
            if marker==c && n>=len && t[n..].trim().is_empty(){ fence=None; }
            continue;
        }
        if n>=3 {fence=Some((marker,n));continue;}
        if html_pre { if t.contains("</pre>"){html_pre=false;} continue; }
        if t.starts_with("<pre") {html_pre=!t.contains("</pre>");continue;}
        if t.starts_with("FILE:") {edit_block=true;continue;}
        if edit_block {
            if t.starts_with(">>>>>>> REPLACE") {edit_block=false;}
            continue;
        }
        if t.starts_with("<<<<<<< SEARCH")||t=="======="||t.starts_with(">>>>>>> REPLACE") {continue;}
        if t.starts_with("RUN:")||t.starts_with("NEED:")||t.starts_with("DELETE:"){continue;}
        result.push(line);
    }
    result.join("\n").trim().to_string()
}

// Explicitly paint each visual row to the transcript width. Background on only
// the text span would otherwise leave a jagged highlight for shorter lines.
fn user_message_rows(s:&str, width:usize)->Vec<Line<'static>> {
    let width=width.max(1);
    let bg=Style::default().bg(Color::Rgb(48,48,48)).fg(Color::White);
    let mut rows=Vec::new();
    for (index,line) in s.split('\n').enumerate() {
        let prefix=if index==0 {"  › "} else {"    "};
        let mut current=prefix.to_string();
        let mut col=prefix.chars().count();
        for ch in line.chars() {
            let char_width=UnicodeWidthChar::width(ch).unwrap_or(0);
            if col+char_width>width && col>0 {
                let pad=width.saturating_sub(col);
                current.push_str(&" ".repeat(pad));
                rows.push(Line::from(Span::styled(current,bg)));
                current=String::new();col=0;
            }
            current.push(ch);col+=char_width;
        }
        current.push_str(&" ".repeat(width.saturating_sub(col)));
        rows.push(Line::from(Span::styled(current,bg)));
    }
    rows
}

// Codex-like transcript formatting: present model Markdown as readable terminal
// prose rather than printing the raw Markdown and its whitespace verbatim.
// Code fences have already been removed by hide_code_blocks.
fn plain_inline(source: &str) -> String {
    let mut out = String::new();
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if (c == '*' || c == '_' || c == '`') && chars.peek() == Some(&c) {
            chars.next();
            continue;
        }
        if c == '*' || c == '`' { continue; }
        if c == '[' {
            let mut text = String::new();
            while let Some(&n) = chars.peek() {
                if n == ']' { break; }
                text.push(n); chars.next();
            }
            if chars.peek() == Some(&']') {
                chars.next();
                if chars.peek() == Some(&'(') {
                    chars.next();
                    let mut depth = 1usize;
                    while let Some(n) = chars.next() {
                        if n == '(' { depth += 1; }
                        if n == ')' { depth -= 1; if depth == 0 { break; } }
                    }
                    out.push_str(&text);
                    continue;
                }
                out.push('['); out.push_str(&text); out.push(']');
                continue;
            }
            out.push('['); out.push_str(&text);
            continue;
        }
        if c == '\\' && matches!(chars.peek().copied(),Some('`'|'*'|'_'|'['|']'|'#')) {
            if let Some(n) = chars.next() { out.push(n); }
            continue;
        }
        out.push(c);
    }
    out
}
fn markdown_rule(line: &str) -> bool {
    let s = line.trim();
    s.len() >= 3 && s.chars().all(|c| c == '-' || c == '*' || c == '_' || c == ' ')
        && s.chars().filter(|c| *c != ' ').count() >= 3
}
fn list_marker(line: &str) -> Option<(&'static str, &str)> {
    let trimmed = line.trim_start();
    for symbol in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = trimmed.strip_prefix(symbol) { return Some(("•",rest)); }
    }
    let digits = trimmed.bytes().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 5 {
        let rest = &trimmed[digits..];
        if let Some(rest) = rest.strip_prefix(". ").or_else(||rest.strip_prefix(") ")) {
            return Some(("#",rest));
        }
    }
    None
}
fn assistant_message_lines(source: &str) -> Vec<Line<'static>> {
    let mut out = Vec::<Line<'static>>::new();
    let mut paragraph = String::new();
    let mut pending_break = false;
    let flush = |out: &mut Vec<Line<'static>>, paragraph: &mut String, pending_break: &mut bool| {
        if !paragraph.is_empty() {
            if *pending_break && !out.is_empty() && out.last().is_some_and(|last| last.width()>0) {out.push(Line::from(""));}
            out.push(Line::from(Span::raw(format!("  {}",plain_inline(paragraph)))));
            paragraph.clear();
            *pending_break = false;
        }
    };
    for original in source.lines() {
        let s=original.trim();
        if s.is_empty() {
            flush(&mut out,&mut paragraph,&mut pending_break);
            pending_break=true;
            continue;
        }
        if markdown_rule(s) {
            flush(&mut out,&mut paragraph,&mut pending_break);
            // A thematic break needs no literal ---- text or additional vertical space.
            pending_break=true;
            continue;
        }
        if let Some(title)=s.strip_prefix('#') {
            let title=title.trim_start_matches('#').trim_start();
            if !title.is_empty() && (s.starts_with("# ") || s.starts_with("## ") || s.starts_with("### ") || s.starts_with("#### ") || s.starts_with("##### ")) {
                flush(&mut out,&mut paragraph,&mut pending_break);
                if !out.is_empty() && out.last().is_some_and(|last| last.width()>0) {out.push(Line::from(""));}
                out.push(Line::from(Span::styled(format!("  {}",plain_inline(title)),Style::default().add_modifier(Modifier::BOLD))));
                pending_break=false;
                continue;
            }
        }
        if let Some((marker,content))=list_marker(s) {
            flush(&mut out,&mut paragraph,&mut pending_break);
            if pending_break && !out.is_empty() && out.last().is_some_and(|last| last.width()>0) {out.push(Line::from(""));}
            let prefix=if marker=="#" {
                let item=s.split_once(|c:char| c=='.'||c==')').map(|(n,_)|n).unwrap_or("1");
                format!("  {item}. ")
            } else {"  • ".to_string()};
            out.push(Line::from(vec![Span::styled(prefix,Style::default().fg(Color::DarkGray)), Span::raw(plain_inline(content))]));
            pending_break=false;
            continue;
        }
        if let Some(rest)=s.strip_prefix("> ") {
            flush(&mut out,&mut paragraph,&mut pending_break);
            if pending_break && !out.is_empty() && out.last().is_some_and(|last| last.width()>0) {out.push(Line::from(""));}
            out.push(Line::from(Span::styled(format!("  │ {}",plain_inline(rest)),theme_muted())));
            pending_break=false;
            continue;
        }
        // One hard line break in a model response is generally part of the same
        // paragraph; wrapping is delegated to the transcript renderer.
        if !paragraph.is_empty() {paragraph.push(' ');}
        paragraph.push_str(s);
    }
    flush(&mut out,&mut paragraph,&mut pending_break);
    out
}

fn send(s: &mut TcpStream,kind: &str,body: &str) -> io::Result<()> { serde_json::to_writer(&mut *s, &Outgoing{kind,body}).map_err(io::Error::other)?; s.write_all(b"\n")?; s.flush() }
fn theme_muted()->Style { Style::default().fg(Color::DarkGray) }

// Generate terminal-width rows only when content or terminal size changes.
// Scrolling can then slice a cached viewport in O(visible rows) time.
fn wrap_transcript_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let width=width.max(1);
    let mut result=Vec::new();
    for line in lines {
        let mut row:Vec<Span<'static>>=Vec::new();
        let mut col=0usize;
        for span in line.spans {
            let style=span.style;
            let mut fragment=String::new();
            for ch in span.content.chars() {
                let w=UnicodeWidthChar::width(ch).unwrap_or(0);
                if col+w>width && col>0 {
                    if !fragment.is_empty() {row.push(Span::styled(std::mem::take(&mut fragment),style));}
                    result.push(Line::from(std::mem::take(&mut row)));
                    col=0;
                }
                fragment.push(ch);
                col+=w;
            }
            if !fragment.is_empty(){row.push(Span::styled(fragment,style));}
        }
        result.push(Line::from(row));
    }
    result
}
fn refresh_transcript(app:&mut App,width:usize) {
    if !app.transcript_dirty && app.cache_width==width {return;}
    let mut lines=Vec::<Line>::new();
    if app.entries.is_empty() {
        lines.push(Line::from(""));
        if !app.ready {
            let spinner=["◐", "◓", "◑", "◒"][(app.tick as usize/4)%4];
            lines.push(Line::from(Span::styled(format!("  {spinner}  WebCodex is starting"),Style::default().add_modifier(Modifier::BOLD))));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(format!("  {}",app.status),theme_muted())));
            lines.push(Line::from(Span::styled("  Connecting to ChatGPT in the browser…",theme_muted())));
        } else {lines.push(Line::from(Span::styled("  Send a message to start working in this project.", theme_muted())));}
    }
    for e in &app.entries {
        if !lines.is_empty() && lines.last().is_some_and(|line| line.width()>0) {
            lines.push(Line::from(""));
        }
        if e.kind=="user" {
            lines.extend(user_message_rows(e.text.trim_end_matches('\n'),width));
            continue;
        }
        if e.kind=="assistant" || e.kind=="stream" {
            lines.extend(assistant_message_lines(&e.text));
            continue;
        }
        // Activity and progress belong to one quiet status line, not a large
        // Markdown paragraph or an independently spaced transcript section.
        if e.kind=="activity" || e.kind=="output" || e.kind=="system" {
            if lines.last().is_some_and(|line| line.width()==0) {lines.pop();}
            let message=e.text.trim();
            if !message.is_empty() {
                lines.push(Line::from(Span::styled(format!("  · {}",plain_inline(message)),theme_muted())));
            }
            continue;
        }
        let is_error=e.kind=="error";
        let prefix=if is_error {"  ! "} else {"  "};
        let style=if is_error {Style::default().fg(Color::Red)} else {theme_muted()};
        for l in e.text.lines().filter(|l|!l.trim().is_empty()) {
            lines.push(Line::from(Span::styled(format!("{prefix}{}",plain_inline(l.trim())),style)));
        }
    }
    app.cached_rows=wrap_transcript_lines(lines,width);
    app.cache_width=width;
    app.transcript_dirty=false;
}

fn render(f: &mut ratatui::Frame,app: &mut App) {
    let area=f.area();
    // Paint the entire frame, not just the regions that contain text. This
    // prevents ghost characters in terminals that keep old cell contents when
    // a wrapped line becomes shorter or the viewport changes.
    f.render_widget(Clear, area);
    let rows=Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(4),Constraint::Min(3),Constraint::Length(1),Constraint::Length(4),Constraint::Length(1)]).split(area);
    let header=Text::from(vec![Line::from(vec![Span::styled("  ╭─ ",theme_muted()),Span::styled("WebCodex",Style::default().add_modifier(Modifier::BOLD)),Span::styled(" ───────────────────────────────────────────────────╮",theme_muted())]),Line::from(Span::styled("  │  Coding agent",theme_muted())),Line::from(Span::styled(format!("  │  {}",app.project),theme_muted())),Line::from(Span::styled("  ╰────────────────────────────────────────────────────────────╯",theme_muted()))]);
    f.render_widget(Paragraph::new(header),rows[0]);
    let height=rows[1].height as usize;
    // Keep the transcript off the terminal's final column. A wide glyph or a
    // full-width highlighted row there can trigger implicit wrap in ConPTY,
    // shifting subsequent output and leaving ghost characters on redraw.
    let transcript_area = Rect { width: rows[1].width.saturating_sub(1).max(1), ..rows[1] };
    let width=transcript_area.width as usize;
    refresh_transcript(app,width);
    let max_scroll=app.cached_rows.len().saturating_sub(height).min(u16::MAX as usize) as u16;
    app.max_scroll=max_scroll;
    if app.tail {app.scroll=max_scroll;} else {app.scroll=app.scroll.min(max_scroll);}
    let start=app.scroll as usize;
    let end=start.saturating_add(height).min(app.cached_rows.len());
    let visible=if start<end {app.cached_rows[start..end].to_vec()} else {Vec::new()};
    f.render_widget(Paragraph::new(visible),transcript_area);
    if max_scroll>0 && rows[1].width>2 {
        let mut state=ScrollbarState::new(max_scroll as usize).position(app.scroll as usize);
        f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(Color::DarkGray))
            .track_style(Style::default().fg(Color::Rgb(35,35,35))),rows[1],&mut state);
    }
    if !app.tail && max_scroll>0 && rows[1].height>2 && rows[1].width>28 {
        let label=" ↓ Jump to latest (End) ";
        let w=label.len() as u16;
        let jump=Rect{x:rows[1].right().saturating_sub(w+3),y:rows[1].bottom().saturating_sub(2),width:w,height:1};
        f.render_widget(Paragraph::new(Span::styled(label,Style::default().fg(Color::White).bg(Color::Rgb(55,55,55)))),jump);
    }
    let busy=if app.busy {"● Working…"} else if !app.ready {&app.status} else {""};
    f.render_widget(Paragraph::new(Span::styled(format!("  {busy}"),theme_muted())),rows[2]);
    let border=Block::default().borders(Borders::ALL).border_style(theme_muted());
    let composer=Paragraph::new(format!("› {}",app.draft)).wrap(Wrap{trim:false}).block(border);
    f.render_widget(composer,rows[3]);
    // Keep the editor cursor visible as text is entered and edited.
    let inner_width=rows[3].width.saturating_sub(2).max(1) as usize;
    let before_cursor=&app.draft[..app.cursor];
    let cursor_line=before_cursor.split('\n').count().saturating_sub(1);
    let last_line=before_cursor.rsplit('\n').next().unwrap_or("");
    let cursor_col=1+last_line.chars().count();
    let wrapped=cursor_col.saturating_sub(1)/inner_width;
    let y=rows[3].y.saturating_add(1).saturating_add((cursor_line+wrapped).min(u16::MAX as usize) as u16);
    let x=rows[3].x.saturating_add(1).saturating_add((cursor_col%inner_width) as u16);
    if y<rows[3].bottom().saturating_sub(1){f.set_cursor_position((x,y));}
    let footer=if app.approval.is_some() {"  Y allow  ·  N deny  ·  A always allow"} else {"  Enter send · ↑ history · PgUp/PgDn scroll · End latest · Alt+Enter newline · Ctrl+Q quit"};
    f.render_widget(Paragraph::new(Span::styled(footer,theme_muted())),rows[4]);
    if let Some(question)=&app.approval {
        let w=area.width.saturating_sub(8).min(85).max(15);
        let h=7.min(area.height);
        let popup=Rect{x:area.x+(area.width.saturating_sub(w))/2,y:area.y+(area.height.saturating_sub(h))/2,width:w,height:h};
        f.render_widget(Clear,popup);
        f.render_widget(Paragraph::new(format!("\n{}\n\n[Y] Allow    [N] Deny    [A] Always",question)).wrap(Wrap{trim:true}).block(Block::default().title(" Approval required ").borders(Borders::ALL)),popup);
    }
}
fn on_message(app:&mut App, m:Incoming) {match m.kind.as_str() {
    "busy"=>app.busy=m.body=="true", "status"=>{app.status=m.body;if app.entries.is_empty(){app.transcript_dirty=true;}}, "ready"=>{app.ready=true;app.status.clear();if app.entries.is_empty(){app.transcript_dirty=true;}}, "approve"=>app.approval=Some(m.body), "exit"=>app.exit=true,
    "stream"=>app.stream(m.body), "stream_end"=>app.end_stream(m.body),
    "history"=>{if let Ok(messages)=serde_json::from_str::<Vec<(String,String)>>(&m.body){app.load_history(&messages);}},
    "user"|"assistant"|"error"|"activity"|"output"|"system"|"clear"=>app.push(m.kind,if m.body.is_empty(){m.title}else{m.body}),
    _=>{}
}}
fn run(port: u16,project: String)->io::Result<()> {
    let addr=SocketAddr::from(([127,0,0,1],port));
    let mut sock=TcpStream::connect_timeout(&addr,Duration::from_secs(10))?;
    sock.set_nodelay(true)?;
    let reader=sock.try_clone()?;
    let (tx,rx):(mpsc::Sender<Incoming>,Receiver<Incoming>)=mpsc::channel();
    thread::spawn(move || {
        let mut r=BufReader::new(reader);
        loop {
            let mut buf=String::new();
            match r.read_line(&mut buf){
                Ok(0)=>break,
                Err(e) if e.kind()==io::ErrorKind::WouldBlock || e.kind()==io::ErrorKind::Interrupted => {
                    thread::sleep(Duration::from_millis(15));continue;
                }
                Err(_)=>break,
                Ok(_)=>if let Ok(m)=serde_json::from_str::<Incoming>(&buf){
                    if tx.send(m).is_err(){return;}
                }
            }
        }
        // Don't leave an unresponsive terminal UI open if the native agent exits.
        let _=tx.send(Incoming{kind:"exit".into(),title:String::new(),body:String::new()});
    });
    enable_raw_mode()?;
    let mut out=io::stdout();
    if let Err(e)=execute!(out,EnterAlternateScreen,event::EnableMouseCapture){
        let _=disable_raw_mode();
        return Err(e);
    }
    let backend=CrosstermBackend::new(out);
    let result=match Terminal::new(backend){
        Ok(mut terminal)=>{
            let result=terminal.clear().and_then(|_|loop_ui(&mut terminal,&mut sock,rx,project));
            let _=terminal.show_cursor();
            result
        }
        Err(e)=>Err(e),
    };
    // Restore the shell even when drawing or socket I/O fails.
    let _=disable_raw_mode();
    let _=execute!(io::stdout(),event::DisableMouseCapture,LeaveAlternateScreen);
    result
}
fn loop_ui(terminal:&mut Terminal<CrosstermBackend<io::Stdout>>, sock:&mut TcpStream,rx:Receiver<Incoming>,project:String)->io::Result<()> {
    let mut app=App::new(project);
    let mut redraw=true;
    // Full terminal invalidation is reserved for transcript/viewport changes.
    // Composer typing stays incremental for low latency.
    let mut full_repaint=true;
    loop {
        while let Ok(m)=rx.try_recv(){
            let kind=m.kind.clone();
            on_message(&mut app,m);
            redraw=true;
            if matches!(kind.as_str(),"history"|"stream"|"stream_end"|"user"|"assistant"|"activity"|"output"|"system"|"error"|"clear"|"ready") {
                full_repaint=true;
            }
        }
        if app.exit {break;}
        if redraw {
            // ratatui normally diffs against its previous frame. After an
            // external console write or a reflow, that diff can retain stale
            // cells. Reset its backbuffer before repainting the visible UI.
            if full_repaint {terminal.clear()?;full_repaint=false;}
            terminal.draw(|f|render(f,&mut app))?;
            redraw=false;
        }
        if !event::poll(Duration::from_millis(80))? {
            if !app.ready && app.entries.is_empty() {app.tick=app.tick.wrapping_add(1);app.transcript_dirty=true;redraw=true;}
            continue;
        }
        redraw=true;
        match event::read()? {
            Event::Resize(_,_)=>{app.transcript_dirty=true;full_repaint=true;},
            Event::Mouse(m)=>match m.kind { MouseEventKind::ScrollUp=>{app.move_scroll(-3);full_repaint=true;},MouseEventKind::ScrollDown=>{app.move_scroll(3);full_repaint=true;},
                MouseEventKind::Down(crossterm::event::MouseButton::Left)=>{
                    let size=terminal.size()?;
                    if !app.tail && m.row>=size.height.saturating_sub(8) && m.row<size.height.saturating_sub(5)
                        && m.column>size.width.saturating_sub(35){app.jump_bottom();full_repaint=true;}
                    else if m.column>=size.width.saturating_sub(2) && m.row>=4 && m.row<size.height.saturating_sub(6){
                        let section=size.height.saturating_sub(10).max(1) as u32;
                        app.scroll=((m.row.saturating_sub(4) as u32 * app.max_scroll as u32)/section) as u16;
                        app.tail=app.scroll>=app.max_scroll;
                        full_repaint=true;
                    }
                },_=>{} },
            Event::Key(KeyEvent{code,modifiers,kind,..})=>{
                // Windows Terminal reports press/repeat/release independently.
                // Release events were being inserted as duplicate characters.
                if kind == KeyEventKind::Release {continue;}
                if app.approval.is_some(){
                    let ans=match code {KeyCode::Char('y')|KeyCode::Char('Y')=>Some("y"),KeyCode::Char('n')|KeyCode::Char('N')|KeyCode::Esc=>Some("n"),KeyCode::Char('a')|KeyCode::Char('A')=>Some("a"),_=>None};
                    if let Some(a)=ans {send(sock,"approve",a)?;app.approval=None;}
                    continue;
                }
                match code {
                    KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL)=>{send(sock,"exit","")?;break;},
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL)=>{app.draft.clear();app.cursor=0;app.history_index=None;},
                    KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL)=>{app.entries.clear();app.transcript_dirty=true;app.jump_bottom();full_repaint=true;},
                    KeyCode::PageUp=>{app.move_scroll(-20);full_repaint=true;},KeyCode::PageDown=>{app.move_scroll(20);full_repaint=true;},
                    KeyCode::Up if modifiers.contains(KeyModifiers::CONTROL)=>{app.move_scroll(-4);full_repaint=true;},
                    KeyCode::Down if modifiers.contains(KeyModifiers::CONTROL)=>{app.move_scroll(4);full_repaint=true;},
                    KeyCode::End if app.draft.is_empty()=>{app.jump_bottom();full_repaint=true;},
                    KeyCode::Up if !modifiers.contains(KeyModifiers::ALT) && !app.draft.contains('\n')=>app.recall_prev(),
                    KeyCode::Down if !modifiers.contains(KeyModifiers::ALT) && !app.draft.contains('\n')=>app.recall_next(),
                    KeyCode::Enter if modifiers.contains(KeyModifiers::ALT)=>app.put('\n'),
                    KeyCode::Enter=>if app.ready && !app.busy && !app.draft.trim().is_empty(){
                        let task=app.draft.trim().to_string();send(sock,"task",&task)?;
                        app.submit(task);
                    },
                    KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL)=>{app.put(c);},
                    KeyCode::Backspace=>app.backspace(), KeyCode::Delete=>app.delete_forward(),
                    KeyCode::Left=>if app.cursor>0 {app.cursor=app.draft[..app.cursor].char_indices().last().map(|(i,_)|i).unwrap_or(0)},
                    KeyCode::Right=>if app.cursor<app.draft.len(){app.cursor+=app.draft[app.cursor..].chars().next().unwrap().len_utf8()},
                    KeyCode::Home=>app.cursor=0, KeyCode::End=>app.cursor=app.draft.len(),
                    _=>{},
                }
            },_=>{}
        }
    }
    Ok(())
}
fn main(){let mut args=env::args().skip(1);let port=args.next().and_then(|s|s.parse().ok());let project=args.next().unwrap_or_else(||"WebCodex".into());match port{Some(p)=>if let Err(e)=run(p,project){eprintln!("WebCodex terminal error: {e}");std::process::exit(1)},None=>{eprintln!("Usage: webcodex-tui <port> <project>");std::process::exit(2)}}}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn streaming_updates_one_transcript_entry() {
        let mut app=App::new("project".into());
        on_message(&mut app, Incoming{kind:"stream".into(),title:String::new(),body:"first".into()});
        on_message(&mut app, Incoming{kind:"stream".into(),title:String::new(),body:"first second".into()});
        assert_eq!(app.entries.len(),1);
        assert_eq!(app.entries[0].text,"first second");
        on_message(&mut app, Incoming{kind:"stream_end".into(),title:String::new(),body:"final".into()});
        assert_eq!(app.entries.len(),1);
        assert_eq!(app.entries[0].kind,"assistant");
        assert_eq!(app.entries[0].text,"final");
    }
    #[test]
    fn user_lines_have_gray_background_and_full_width() {
        let rows=user_message_rows("Hello\nWorld",32);
        assert_eq!(rows.len(),2);
        assert_eq!(rows[0].width(),32);
        assert_eq!(rows[1].width(),32);
        assert_eq!(rows[0].spans[0].style.bg,Some(Color::Rgb(48,48,48)));
    }
    #[test]
    fn user_long_messages_wrap_without_overflow() {
        let rows=user_message_rows("abcdefghijklmnopqrstuvwxyz",12);
        assert!(rows.len()>1);
        assert!(rows.iter().all(|row| row.width()==12));
    }
    #[test]
    fn code_blocks_hidden_even_during_streaming() {
        let mut app=App::new("project".into());
        app.stream("Hello\n```rust\nlet secret=1;".into());
        assert_eq!(app.entries[0].text,"Hello");
        app.end_stream("Hello\n```rust\nlet secret=1;\n```\nDone".into());
        assert_eq!(app.entries[0].text,"Hello\nDone");
    }
    #[test]
    fn clearing_transcript_restores_tail_mode() {
        let mut app=App::new("project".into());
        app.push("assistant".into(),"before".into());
        app.move_scroll(-3);
        app.push("clear".into(),String::new());
        assert!(app.entries.is_empty());
        assert!(app.tail);
    }
    #[test]
    fn history_up_down_restores_unsent_draft() {
        let mut app=App::new("p".into());
        app.submit("first".into());app.busy=false;
        app.submit("second".into());app.busy=false;
        app.set_draft("unsent".into());
        app.recall_prev();assert_eq!(app.draft,"second");
        app.recall_prev();assert_eq!(app.draft,"first");
        app.recall_next();assert_eq!(app.draft,"second");
        app.recall_next();assert_eq!(app.draft,"unsent");
    }
    #[test]
    fn scrollback_pauses_follow_and_bottom_resumes() {
        let mut app=App::new("p".into());app.max_scroll=100;app.jump_bottom();
        app.move_scroll(-8);assert!(!app.tail);
        app.push("assistant".into(),"new".into());assert!(!app.tail);
        app.jump_bottom();assert!(app.tail);
    }
    #[test]
    fn markdown_is_compact_and_not_raw() {
        let text="\n\n## Add dedicated tests\n\n\n**All 14 original tests still pass.**\n\n---\n\nNow we verify communication.\n\n";
        let rendered=assistant_message_lines(text);
        let visible=rendered.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>()).collect::<Vec<_>>().join("\n");
        assert!(visible.contains("Add dedicated tests"));
        assert!(visible.contains("All 14 original tests still pass."));
        assert!(!visible.contains("##"));
        assert!(!visible.contains("**"));
        assert!(!visible.contains("---"));
        assert!(!visible.contains("\n\n\n"));
    }
    #[test]
    fn markdown_lists_and_links() {
        let rows=assistant_message_lines("- **Fixed** the [socket](https://example.com)\n- Ran tests\n\n1. First step\n2. Next step");
        let s=rows.iter().map(|l| l.spans.iter().map(|x| x.content.as_ref()).collect::<String>()).collect::<Vec<_>>().join("\n");
        assert!(s.contains("• Fixed the socket"));
        assert!(s.contains("2. Next step"));
        assert!(!s.contains("https://"));
    }
    #[test]
    fn strips_unfenced_patch_blocks_and_html_pre() {
        let text="Intro\nFILE: a.py\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\n<pre>code</pre>\nDone";
        assert_eq!(hide_code_blocks(text),"Intro\nDone");
    }

    #[test]
    fn model_control_sequences_cannot_move_terminal_cursor() {
        let cleaned=sanitize_display("one\u{1b}[12;40Htwo\u{1b}]0;title\u{7}three\rOK");
        assert_eq!(cleaned,"onetwothree\nOK");
    }
    #[test]
    fn assistant_stream_is_sanitized_before_rendering() {
        let mut app=App::new("p".into());
        app.stream("hello\u{1b}[2Jworld".into());
        assert_eq!(app.entries[0].text,"helloworld");
    }

    #[test]
    fn cached_transcript_does_not_rebuild_for_scroll() {
        let mut app=App::new("p".into());
        app.push("assistant".into(),"A long response ".repeat(500));
        refresh_transcript(&mut app,80);
        assert!(!app.transcript_dirty);
        let count=app.cached_rows.len();
        app.max_scroll=100;app.move_scroll(-10);
        refresh_transcript(&mut app,80);
        assert_eq!(app.cached_rows.len(),count);
        assert!(!app.transcript_dirty);
        refresh_transcript(&mut app,40);
        assert!(app.cached_rows.len()>count);
    }
    #[test]
    fn history_batch_adds_entries_without_clobbering_draft() {
        let mut app=App::new("p".into());
        app.set_draft("unsent".into());
        app.load_history(&[("user".into(),"hi".into()),("assistant".into(),"hello".into())]);
        assert_eq!(app.entries.len(),2);
        assert_eq!(app.draft,"unsent");
        assert_eq!(app.history,["hi"]);
    }

}
