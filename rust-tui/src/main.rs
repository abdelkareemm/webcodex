//! WebCodex TUI. Inspired by the ratatui-based OpenAI Codex terminal frontend.
//! WebCodex's existing Python browser transport remains the execution engine.
use std::{env, io::{self, BufRead, BufReader, Write}, net::{TcpStream, SocketAddr}, sync::mpsc::{self, Receiver}, thread, time::Duration};
use crossterm::{event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind}, execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{backend::CrosstermBackend, layout::{Constraint, Direction, Layout, Rect}, style::{Color, Modifier, Style}, text::{Line, Span, Text}, widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap, Scrollbar, ScrollbarOrientation, ScrollbarState}, Terminal};
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthChar;

mod command_palette;

#[derive(Debug, Deserialize)]
struct Incoming { kind: String, #[serde(default)] title: String, #[serde(default)] body: String }
#[derive(Serialize)]
struct Outgoing<'a> { kind: &'a str, body: &'a str }
#[derive(Clone)]
struct Entry { kind: String, text: String }
#[derive(Clone, Debug, Deserialize)]
struct SettingsView {
    auto_apply: bool,
    allow_run: bool,
    approval_never: bool,
    max_turns: usize,
}
struct App { cached_rows: Vec<Line<'static>>, cache_width: usize, transcript_dirty: bool, entries: Vec<Entry>, draft: String, cursor: usize, scroll: u16, tail: bool, busy: bool, approval: Option<String>, settings: Option<SettingsView>, settings_index: usize, page: Option<PageView>, exit: bool, project: String, history: Vec<String>, history_index: Option<usize>, history_stash: String, status: String, ready: bool, max_scroll: u16, tick: u64, palette_index: usize }
#[derive(Clone, Debug, Deserialize)]
struct SessionChoice {
    id: String,
    title: String,
    updated: String,
    active: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct PageView {
    title: String,
    lines: Vec<String>,
    #[serde(default)]
    choices: Vec<SessionChoice>,
    #[serde(skip)]
    selected: usize,
    #[serde(skip)]
    scroll: usize,
}

impl PageView {
    fn move_scroll(&mut self, delta: i32) {
        self.scroll = self.scroll.saturating_add_signed(delta as isize)
            .min(self.lines.len().saturating_sub(1));
    }

    fn select_relative(&mut self, delta: isize) {
        if self.choices.is_empty() {
            self.selected=0;
            self.scroll=0;
            return;
        }
        self.selected=self.selected.saturating_add_signed(delta)
            .min(self.choices.len()-1);
        self.scroll=self.selected;
    }

    fn selected_session_id(&self)->Option<&str>{
        self.choices.get(self.selected).map(|choice|choice.id.as_str())
    }
}

impl App {
    fn new(project: String) -> Self { Self { cached_rows: Vec::new(), cache_width: 0, transcript_dirty: true, entries: vec![], draft: String::new(), cursor: 0, scroll: 0, tail: true, busy: false, approval: None, settings: None, settings_index: 0, page: None, exit: false, project, history: Vec::new(), history_index: None, history_stash: String::new(), status: "Starting WebCodex…".into(), ready: false, max_scroll: 0, tick: 0, palette_index: 0 } }
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
    fn set_draft(&mut self, text:String) {
        self.draft=text;
        self.cursor=self.draft.len();
        self.palette_index=0;
    }
    fn selected_setting_command(&self)->Option<&'static str>{
        let settings=self.settings.as_ref()?;
        match self.settings_index {
            0=>Some("/permissions ask"),
            1=>Some("/permissions auto"),
            2=>Some(if settings.allow_run {
                "/permissions commands-off"
            }else{
                "/permissions commands-on"
            }),
            3=>Some("/turns 25"),
            4=>Some("/turns 100"),
            5=>Some("/turns 250"),
            6=>Some("/turns 500"),
            _=>None,
        }
    }
    fn enter_can_complete_palette(&self)->bool {
        self.draft.starts_with('/') && self.draft.split_whitespace().count()==1
    }

    fn complete_palette(&mut self)->bool {
        if !self.draft.starts_with('/') {return false;}
        let matches=command_palette::search(&self.draft);
        let Some(command)=matches.get(self.palette_index.min(matches.len().saturating_sub(1))) else {
            return false;
        };
        if self.draft==command.name {return false;}
        self.set_draft(command.name.to_string());
        true
    }
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
        self.history_index=None;
        self.history_stash.clear();
        self.push("user".into(),task);
        self.set_draft(String::new());
        self.jump_bottom();
    }
    fn put(&mut self, c:char) {
        self.draft.insert(self.cursor,c);
        self.cursor+=c.len_utf8();
        self.palette_index=0;
    }
    fn backspace(&mut self) {
        if self.cursor>0 {
            let i=self.draft[..self.cursor].char_indices().last().map(|(i,_)|i).unwrap_or(0);
            self.draft.drain(i..self.cursor);
            self.cursor=i;
            self.palette_index=0;
        }
    }
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

fn compact_path(path:&str, max_width:usize)->String {
    if max_width==0 {return String::new();}
    if path.chars().count()<=max_width {return path.to_string();}
    if max_width<=3 {return ".".repeat(max_width);}
    let suffix=path.chars().rev().take(max_width-3).collect::<Vec<_>>();
    format!("...{}",suffix.into_iter().rev().collect::<String>())
}

fn composer_cursor_position(draft:&str, cursor:usize, width:usize)->(usize,usize) {
    let width=width.max(1);
    let mut row=0usize;
    let mut col=0usize;
    let cursor=cursor.min(draft.len());
    let cursor=if draft.is_char_boundary(cursor) {
        cursor
    } else {
        (0..cursor).rev().find(|&i|draft.is_char_boundary(i)).unwrap_or(0)
    };

    for ch in "› ".chars().chain(draft[..cursor].chars()) {
        if ch=='\n' {
            row+=1;
            col=0;
            continue;
        }
        let size=UnicodeWidthChar::width(ch).unwrap_or(0);
        if col+size>width {
            row+=1;
            col=0;
        }
        col+=size;
        if col>=width {
            row+=1;
            col=0;
        }
    }
    (row,col)
}

fn composer_visual_rows(draft:&str, width:usize)->Vec<String> {
    let width=width.max(1);
    let mut rows=Vec::new();
    let mut current=String::new();
    let mut col=0usize;

    for ch in "› ".chars().chain(draft.chars()) {
        if ch=='\n' {
            rows.push(std::mem::take(&mut current));
            col=0;
            continue;
        }
        let size=UnicodeWidthChar::width(ch).unwrap_or(0);
        if col+size>width && col>0 {
            rows.push(std::mem::take(&mut current));
            col=0;
        }
        current.push(ch);
        col+=size;
        if col>=width {
            rows.push(std::mem::take(&mut current));
            col=0;
        }
    }

    rows.push(current);
    rows
}

fn composer_height(draft:&str, terminal_width:u16, terminal_height:u16)->u16 {
    let content_width=terminal_width.saturating_sub(3).max(1) as usize;
    let mut rows=0usize;

    for (index,line) in draft.split('\n').enumerate() {
        let text_width=line.chars()
            .map(|ch|UnicodeWidthChar::width(ch).unwrap_or(0))
            .sum::<usize>()
            + if index==0 {2}else{0};
        rows+=text_width.saturating_sub(1)/content_width+1;
    }

    let maximum=terminal_height.saturating_sub(8).clamp(4,12);
    rows.saturating_add(2).clamp(4,maximum as usize) as u16
}

fn header_lines(project:&str, width:usize, busy:bool, ready:bool)->Text<'static> {
    let state=if busy {"WORKING"}else if ready {"READY"}else{"CONNECTING"};
    let label=format!("  WebCodex  /  {state}");
    let label=compact_path(&label,width);
    let prefix=if width>=2 {"  "}else{""};
    let path=compact_path(project,width.saturating_sub(prefix.len()));
    Text::from(vec![
        Line::from(Span::styled(label,Style::default().add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("{prefix}{path}"),theme_muted())),
    ])
}

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
    let input_height=composer_height(&app.draft,area.width,area.height);
    let rows=Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(2),Constraint::Min(3),Constraint::Length(1),Constraint::Length(input_height),Constraint::Length(1)]).split(area);
    let header=Text::from(vec![Line::from(vec![Span::styled("  ╭─ ",theme_muted()),Span::styled("WebCodex",Style::default().add_modifier(Modifier::BOLD)),Span::styled(" ───────────────────────────────────────────────────╮",theme_muted())]),Line::from(Span::styled("  │  Coding agent",theme_muted())),Line::from(Span::styled(format!("  │  {}",app.project),theme_muted())),Line::from(Span::styled("  ╰────────────────────────────────────────────────────────────╯",theme_muted()))]);
    let _=header;
    f.render_widget(
        Paragraph::new(header_lines(&app.project,rows[0].width as usize,app.busy,app.ready)),
        rows[0],
    );
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
        let w=label.chars()
            .map(|ch|UnicodeWidthChar::width(ch).unwrap_or(0))
            .sum::<usize>() as u16;
        let jump=Rect{x:rows[1].right().saturating_sub(w+3),y:rows[1].bottom().saturating_sub(2),width:w,height:1};
        f.render_widget(Paragraph::new(Span::styled(label,Style::default().fg(Color::White).bg(Color::Rgb(55,55,55)))),jump);
    }
    let busy=if app.busy {"● Working…"} else if !app.ready {&app.status} else {""};
    f.render_widget(Paragraph::new(Span::styled(format!("  {busy}"),theme_muted())),rows[2]);
    let border=Block::default().borders(Borders::ALL).padding(Padding::left(1)).border_style(theme_muted());
    let editor_width=rows[3].width.saturating_sub(3).max(1) as usize;
    let (cursor_row,cursor_col)=composer_cursor_position(&app.draft,app.cursor,editor_width);
    let visible_rows=rows[3].height.saturating_sub(2).max(1) as usize;
    let scroll_offset=cursor_row.saturating_sub(visible_rows.saturating_sub(1));
    let composer=Paragraph::new(composer_visual_rows(&app.draft,editor_width).join("\n"))
        .scroll((scroll_offset.min(u16::MAX as usize) as u16,0))
        .block(border);
    f.render_widget(composer,rows[3]);
    // Keep the editor cursor inside the visible input viewport.
    let visible_row=cursor_row.saturating_sub(scroll_offset);
    let y=rows[3].y.saturating_add(1).saturating_add(visible_row as u16);
    let x=rows[3].x.saturating_add(2).saturating_add(cursor_col as u16);
    if y<rows[3].bottom().saturating_sub(1)
        && x<rows[3].right().saturating_sub(1) {
        f.set_cursor_position((x,y));
    }
    let footer=if app.approval.is_some() {"  Y allow  ·  N deny  ·  A always allow"} else {"  Enter send · ↑ history · PgUp/PgDn scroll · End latest · Alt+Enter newline · Ctrl+Q quit"};
    let footer=if app.draft.starts_with('/') {
        "  Up/Down select  |  Tab complete  |  Enter execute  |  Esc dismiss"
    } else {
        footer
    };
    f.render_widget(Paragraph::new(Span::styled(footer,theme_muted())),rows[4]);
    if app.draft.starts_with('/') && app.approval.is_none()
        && app.settings.is_none() && app.page.is_none()
        && area.width >= 45 && area.height >= 18
    {
        let matches=command_palette::search(&app.draft);
        let count=matches.len().min(7);
        if count>0 {
            let width=area.width.saturating_sub(8).min(80);
            let height=count as u16+2;
            let popup=Rect {
                x:area.x+4,
                y:rows[3].y.saturating_sub(height),
                width,
                height,
            };
            let items=matches.iter().take(count).enumerate().map(|(i,command)| {
                let style=if i==app.palette_index.min(count-1) {
                    Style::default().fg(Color::White).bg(Color::Rgb(55,55,65))
                } else {
                    theme_muted()
                };
                Line::from(Span::styled(
                    format!("  {:<12} {}",command.name,command.description),
                    style
                ))
            }).collect::<Vec<_>>();
            f.render_widget(Clear,popup);
            f.render_widget(
                Paragraph::new(items).block(
                    Block::default().title(" Commands ").borders(Borders::ALL)
                ),
                popup
            );
        }
    }
    if let Some(page)=&app.page {
        let width=area.width.saturating_sub(4).min(86).max(1);
        let height=area.height.saturating_sub(4).min(25).max(1);
        let popup=Rect {
            x:area.x+(area.width.saturating_sub(width))/2,
            y:area.y+(area.height.saturating_sub(height))/2,
            width,
            height,
        };
        let visible=height.saturating_sub(4) as usize;
        let offset=page.scroll.min(page.lines.len().saturating_sub(visible));
        let lines=if page.choices.is_empty(){
            page.lines.iter().skip(offset).take(visible)
                .map(|line|Line::from(line.clone()))
                .collect::<Vec<_>>()
        }else{
            let start=page.scroll.min(page.choices.len().saturating_sub(visible.max(1)));
            page.choices.iter().enumerate().skip(start).take(visible)
                .map(|(index,session)|{
                    let selected=index==page.selected;
                    let marker=if selected {">"}else{" "};
                    let active=if session.active {" [ACTIVE]"}else{""};
                    let label=format!(
                        "  {marker} {}{}  |  {}",
                        session.title,active,session.updated
                    );
                    let style=if selected {
                        Style::default().fg(Color::White).bg(Color::Rgb(55,55,65))
                    }else{
                        theme_muted()
                    };
                    Line::from(Span::styled(label,style))
                })
                .collect::<Vec<_>>()
        };
        f.render_widget(Clear,popup);
        f.render_widget(
            Paragraph::new(lines)
                .block(Block::default()
                    .title(format!(" {} ",page.title))
                    .borders(Borders::ALL)
                    .border_style(theme_muted())),
            popup,
        );
        let footer=Rect {
            x:popup.x.saturating_add(2),
            y:popup.bottom().saturating_sub(2),
            width:popup.width.saturating_sub(4),
            height:1,
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                if page.choices.is_empty(){
                    "Up/Down scroll  |  PgUp/PgDn page  |  Esc close"
                }else{
                    "Up/Down select  |  Enter switch  |  Esc cancel"
                },
                theme_muted())),
            footer,
        );
    }
    if let Some(settings)=&app.settings {
        let width=area.width.saturating_sub(4).min(68).max(1);
        let height=16.min(area.height);
        let popup=Rect {
            x:area.x+(area.width.saturating_sub(width))/2,
            y:area.y+(area.height.saturating_sub(height))/2,
            width,
            height,
        };
        let options=[
            ("Ask before changes", "Require approval for edits and commands"),
            ("Automatic execution", "Unrestricted changes and shell execution"),
            ("Toggle shell commands", "Enable or disable command execution"),
            ("25 turns", "Short autonomous tasks"),
            ("100 turns", "Standard autonomous tasks"),
            ("250 turns", "Extended autonomous tasks"),
            ("500 turns", "Long-running autonomous tasks"),
        ];
        let mut lines=vec![
            Line::from(format!("  File edits: {}",if settings.auto_apply {"automatic"}else{"approval required"})),
            Line::from(format!("  Shell commands: {}",if settings.allow_run {"enabled"}else{"disabled"})),
            Line::from(format!("  Approval prompts: {}",if settings.approval_never {"disabled"}else{"enabled"})),
            Line::from(format!("  Maximum task turns: {}",settings.max_turns)),
            Line::from(""),
        ];
        for (index,(name,description)) in options.iter().enumerate(){
            let style=if app.settings_index==index {
                Style::default().fg(Color::White).bg(Color::Rgb(55,55,65))
            }else{
                theme_muted()
            };
            lines.push(Line::from(Span::styled(format!(
                "  {} {} - {}",if app.settings_index==index {">"}else{" "},name,description
            ),style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Up/Down select  |  Enter apply  |  Esc close",theme_muted())));
        f.render_widget(Clear,popup);
        f.render_widget(
            Paragraph::new(lines).wrap(Wrap{trim:false})
                .block(Block::default().title(" Settings ").borders(Borders::ALL)),
            popup,
        );
    }
    if let Some(question)=&app.approval {
        let w=area.width.saturating_sub(2).min(85).max(1);
        let h=7.min(area.height);
        let popup=Rect{x:area.x+(area.width.saturating_sub(w))/2,y:area.y+(area.height.saturating_sub(h))/2,width:w,height:h};
        f.render_widget(Clear,popup);
        f.render_widget(Paragraph::new(format!("\n{}\n\n[Y] Allow    [N] Deny    [A] Always",question)).wrap(Wrap{trim:true}).block(Block::default().title(" Approval required ").borders(Borders::ALL)),popup);
    }
}
fn on_message(app:&mut App, m:Incoming) {match m.kind.as_str() {
    "busy"=>app.busy=m.body=="true", "status"=>{app.status=m.body;if app.entries.is_empty(){app.transcript_dirty=true;}}, "ready"=>{app.ready=true;app.status.clear();if app.entries.is_empty(){app.transcript_dirty=true;}}, "approve"=>app.approval=Some(m.body), "exit"=>app.exit=true,
    "settings"=>{
        if let Ok(settings)=serde_json::from_str::<SettingsView>(&m.body){
            app.page=None;
            app.settings=Some(settings);
            app.settings_index=0;
        }
    },
    "page"=>{
        if let Ok(mut page)=serde_json::from_str::<PageView>(&m.body){
            page.selected=page.choices.iter()
                .position(|choice|choice.active)
                .unwrap_or(0);
            page.scroll=page.selected;
            app.settings=None;
            app.page=Some(page);
        }
    },
    "session_reset"=>{
        app.entries.clear();
        app.history.clear();
        app.history_index=None;
        app.history_stash.clear();
        app.set_draft(String::new());
        app.page=None;
        app.settings=None;
        app.scroll=0;
        app.max_scroll=0;
        app.tail=true;
        app.transcript_dirty=true;
        app.busy=false;
    },
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
            if matches!(kind.as_str(),"history"|"stream"|"stream_end"|"user"|"assistant"|"activity"|"output"|"system"|"error"|"clear"|"ready"|"page"|"settings"|"session_reset") {
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
            Event::Mouse(m)=>match m.kind {
                MouseEventKind::ScrollUp=>{
                    if let Some(page)=app.page.as_mut(){
                        if page.choices.is_empty(){
                            page.move_scroll(-3);
                        }else{
                            page.select_relative(-3);
                        }
                    }else if app.settings.is_none() && app.approval.is_none(){
                        app.move_scroll(-3);
                    }
                    full_repaint=true;
                },
                MouseEventKind::ScrollDown=>{
                    if let Some(page)=app.page.as_mut(){
                        if page.choices.is_empty(){
                            page.move_scroll(3);
                        }else{
                            page.select_relative(3);
                        }
                    }else if app.settings.is_none() && app.approval.is_none(){
                        app.move_scroll(3);
                    }
                    full_repaint=true;
                },
                MouseEventKind::Down(crossterm::event::MouseButton::Left)=>{
                    if app.page.is_some() || app.settings.is_some() || app.approval.is_some(){
                        continue;
                    }
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
                if code==KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
                    if app.busy {
                        send(sock,"cancel","")?;
                        app.approval=None;
                        app.status="Cancelling active task...".into();
                    }else{
                        send(sock,"exit","")?;
                        break;
                    }
                    continue;
                }
                if app.approval.is_some(){
                    let ans=match code {KeyCode::Char('y')|KeyCode::Char('Y')=>Some("y"),KeyCode::Char('n')|KeyCode::Char('N')|KeyCode::Esc=>Some("n"),KeyCode::Char('a')|KeyCode::Char('A')=>Some("a"),_=>None};
                    if let Some(a)=ans {send(sock,"approve",a)?;app.approval=None;}
                    continue;
                }
                if let Some(page)=app.page.as_mut(){
                    if page.choices.is_empty(){
                        match code {
                            KeyCode::Esc|KeyCode::Enter=>app.page=None,
                            KeyCode::Up=>page.move_scroll(-1),
                            KeyCode::Down=>page.move_scroll(1),
                            KeyCode::PageUp=>page.move_scroll(-12),
                            KeyCode::PageDown=>page.move_scroll(12),
                            KeyCode::Home=>page.scroll=0,
                            KeyCode::End=>page.scroll=page.lines.len().saturating_sub(1),
                            _=>{},
                        }
                    }else{
                        match code {
                            KeyCode::Esc=>app.page=None,
                            KeyCode::Up=>page.select_relative(-1),
                            KeyCode::Down=>page.select_relative(1),
                            KeyCode::PageUp=>page.select_relative(-10),
                            KeyCode::PageDown=>page.select_relative(10),
                            KeyCode::Home=>page.selected=0,
                            KeyCode::End=>page.selected=page.choices.len()-1,
                            KeyCode::Enter=>{
                                if let Some(id)=page.selected_session_id(){
                                    send(sock,"task",&format!("/session {id}"))?;
                                    app.page=None;
                                }
                            },
                            _=>{},
                        }
                        if let Some(page)=app.page.as_mut(){
                            if !page.choices.is_empty(){
                                page.scroll=page.selected;
                            }
                        }
                    }
                    continue;
                }
                if app.settings.is_some(){
                    match code {
                        KeyCode::Esc=>app.settings=None,
                        KeyCode::Up=>app.settings_index=(app.settings_index+6)%7,
                        KeyCode::Down=>app.settings_index=(app.settings_index+1)%7,
                        KeyCode::Enter=>{
                            if let Some(command)=app.selected_setting_command(){
                                send(sock,"task",command)?;
                                app.settings=None;
                                app.set_draft(String::new());
                                app.history_index=None;
                            }
                        },
                        _=>{},
                    }
                    continue;
                }
                match code {
                    KeyCode::Char('q') if modifiers.contains(KeyModifiers::CONTROL)=>{send(sock,"exit","")?;break;},
                    KeyCode::Char('k') if modifiers.contains(KeyModifiers::CONTROL)=>{
                        app.set_draft("/".into());
                        app.palette_index=0;
                    },
                    KeyCode::Esc if app.draft.starts_with('/')=>{
                        app.set_draft(String::new());
                        app.palette_index=0;
                    },
                    KeyCode::Tab if app.draft.starts_with('/')=>{
                        app.complete_palette();
                    },
                    KeyCode::Up if app.draft.starts_with('/')=>{
                        let count=command_palette::search(&app.draft).len().min(7);
                        if count>0 {app.palette_index=(app.palette_index+count-1)%count;}
                    },
                    KeyCode::Down if app.draft.starts_with('/')=>{
                        let count=command_palette::search(&app.draft).len().min(7);
                        if count>0 {app.palette_index=(app.palette_index+1)%count;}
                    },

                    KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL)=>{app.entries.clear();app.transcript_dirty=true;app.jump_bottom();full_repaint=true;},
                    KeyCode::PageUp=>{app.move_scroll(-20);full_repaint=true;},KeyCode::PageDown=>{app.move_scroll(20);full_repaint=true;},
                    KeyCode::Up if modifiers.contains(KeyModifiers::CONTROL)=>{app.move_scroll(-4);full_repaint=true;},
                    KeyCode::Down if modifiers.contains(KeyModifiers::CONTROL)=>{app.move_scroll(4);full_repaint=true;},
                    KeyCode::End if app.draft.is_empty()=>{app.jump_bottom();full_repaint=true;},
                    KeyCode::Up if !modifiers.contains(KeyModifiers::ALT) && !app.draft.contains('\n')=>app.recall_prev(),
                    KeyCode::Down if !modifiers.contains(KeyModifiers::ALT) && !app.draft.contains('\n')=>app.recall_next(),
                    KeyCode::Enter if modifiers.contains(KeyModifiers::ALT)=>app.put('\n'),
                    KeyCode::Enter if app.enter_can_complete_palette() && app.complete_palette()=>{},
                    KeyCode::Enter=>if app.ready && !app.draft.trim().is_empty(){
                        let task=app.draft.trim().to_string();
                        send(sock,"task",&task)?;
                        if app.busy && !(task=="/queue" || task.starts_with("/queue ")) {
                            app.history_index=None;
                            app.history_stash.clear();
                            app.push("system".into(),"Submitting to pending queue...".into());
                            app.set_draft(String::new());
                        } else if task=="/queue" || task.starts_with("/queue ") {
                            app.history_index=None;
                            app.history_stash.clear();
                            app.push("user".into(),task);
                            app.set_draft(String::new());
                        } else {
                            app.history_index=None;
                            app.history_stash.clear();
                            app.set_draft(String::new());
                            app.push("system".into(),"Message submitted to agent.".into());
                        }
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
    fn command_arguments_are_preserved_on_enter(){
        let mut app=App::new("project".into());
        for command in [
            "/queue edit 2 revised message",
            "/queue move 3 1",
            "/skills search graphify",
        ] {
            app.set_draft(command.into());
            assert!(!app.enter_can_complete_palette());
            assert_eq!(app.draft,command);
        }
    }

    #[test]
    fn busy_state_is_controlled_by_native_agent_events(){
        let mut app=App::new("project".into());
        app.ready=true;
        app.submit("Example message".into());
        assert!(!app.busy);

        on_message(&mut app,Incoming{
            kind:"busy".into(),
            title:String::new(),
            body:"true".into(),
        });
        assert!(app.busy);

        on_message(&mut app,Incoming{
            kind:"busy".into(),
            title:String::new(),
            body:"false".into(),
        });
        assert!(!app.busy);
    }

    #[test]
    fn socket_protocol_transmits_complete_json_messages(){
        use std::net::TcpListener;

        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let address=listener.local_addr().unwrap();
        let receiver=std::thread::spawn(move || {
            let (socket,_)=listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut reader=BufReader::new(socket);
            let mut messages=Vec::new();

            for _ in 0..3 {
                let mut line=String::new();
                assert!(reader.read_line(&mut line).unwrap()>0);
                let value:serde_json::Value=serde_json::from_str(&line).unwrap();
                messages.push(value);
            }

            messages
        });

        let mut socket=TcpStream::connect(address).unwrap();
        send(&mut socket,"task","/sessions").unwrap();
        send(&mut socket,"task","Unicode: \u{1F680}\nSecond line").unwrap();
        send(&mut socket,"exit","").unwrap();

        let messages=receiver.join().unwrap();
        assert_eq!(messages.len(),3);
        assert_eq!(messages[0]["kind"],"task");
        assert_eq!(messages[0]["body"],"/sessions");
        assert_eq!(messages[1]["body"],"Unicode: \u{1F680}\nSecond line");
        assert_eq!(messages[2]["kind"],"exit");
    }
    #[test]
    fn session_reset_discards_previous_conversation_state(){
        let mut app=App::new("project".into());
        app.load_history(&[
            ("user".into(),"Old question".into()),
            ("assistant".into(),"Old answer".into()),
        ]);
        app.set_draft("Old draft".into());
        app.page=Some(PageView{
            title:"Saved conversations".into(),
            lines:vec!["Old session".into()],
            choices:vec![],
            selected:0,
            scroll:0,
        });

        on_message(&mut app,Incoming{
            kind:"session_reset".into(),
            title:String::new(),
            body:String::new(),
        });

        assert!(app.entries.is_empty());
        assert!(app.history.is_empty());
        assert!(app.draft.is_empty());
        assert!(app.page.is_none());
        assert!(app.tail);

        app.load_history(&[
            ("user".into(),"New question".into()),
            ("assistant".into(),"New answer".into()),
        ]);
        assert_eq!(app.entries.len(),2);
        assert_eq!(app.history,["New question"]);
    }

    #[test]
    fn page_message_opens_information_screen(){
        let mut app=App::new("project".into());
        on_message(&mut app,Incoming{
            kind:"page".into(),
            title:String::new(),
            body:r#"{"title":"Session status","lines":["Project","Thread","Ready"]}"#.into(),
        });
        assert_eq!(app.page.as_ref().unwrap().title,"Session status");
        assert_eq!(app.page.as_ref().unwrap().lines.len(),3);
        assert_eq!(app.page.as_ref().unwrap().scroll,0);
    }

    #[test]
    fn opening_settings_closes_an_information_page(){
        let mut app=App::new("project".into());
        app.page=Some(PageView{
            title:"Saved conversations".into(),
            lines:vec!["Session one".into()],
            choices:vec![],
            selected:0,
            scroll:0,
        });

        on_message(&mut app,Incoming{
            kind:"settings".into(),
            title:String::new(),
            body:r#"{"auto_apply":false,"allow_run":false,"approval_never":false,"max_turns":100}"#.into(),
        });

        assert!(app.page.is_none());
        assert!(app.settings.is_some());
    }

    #[test]
    fn page_message_closes_settings(){
        let mut app=App::new("project".into());
        app.settings=Some(SettingsView{
            auto_apply:false,
            allow_run:false,
            approval_never:false,
            max_turns:100,
        });
        on_message(&mut app,Incoming{
            kind:"page".into(),
            title:String::new(),
            body:r#"{"title":"Saved conversations","lines":["First session"]}"#.into(),
        });
        assert!(app.settings.is_none());
        assert_eq!(app.page.as_ref().unwrap().title,"Saved conversations");
    }

    #[test]
    fn approval_dialog_fits_inside_small_terminals(){
        for (width,height) in [(10,7),(15,8),(20,10),(40,12)] {
            let backend=ratatui::backend::TestBackend::new(width,height);
            let mut terminal=ratatui::Terminal::new(backend).unwrap();
            let mut app=App::new("test".into());
            app.approval=Some("Apply changes?".into());
            terminal.draw(|frame|render(frame,&mut app)).unwrap();
        }
    }

    #[test]
    fn modal_screens_render_without_panicking(){
        let backend=ratatui::backend::TestBackend::new(60,22);
        let mut terminal=ratatui::Terminal::new(backend).unwrap();
        let mut app=App::new("test project".into());
        app.ready=true;

        on_message(&mut app,Incoming{
            kind:"page".into(),
            title:String::new(),
            body:r#"{"title":"Saved conversations","lines":[],"choices":[{"id":"one","title":"First","updated":"today","active":false},{"id":"two","title":"Second","updated":"today","active":true}]}"#.into(),
        });
        terminal.draw(|frame|render(frame,&mut app)).unwrap();

        on_message(&mut app,Incoming{
            kind:"settings".into(),
            title:String::new(),
            body:r#"{"auto_apply":false,"allow_run":false,"approval_never":false,"max_turns":100}"#.into(),
        });
        terminal.draw(|frame|render(frame,&mut app)).unwrap();
        assert!(app.page.is_none());
        assert!(app.settings.is_some());
    }

    #[test]
    fn structured_sessions_load_with_active_selection(){
        let mut app=App::new("project".into());
        on_message(&mut app,Incoming{
            kind:"page".into(),
            title:String::new(),
            body:r#"{"title":"Saved conversations","lines":[],"choices":[{"id":"one","title":"First task","updated":"2026-11-05","active":true},{"id":"two","title":"Second task","updated":"2026-11-05","active":false}]}"#.into(),
        });
        let page=app.page.as_ref().unwrap();
        assert_eq!(page.choices.len(),2);
        assert_eq!(page.choices[0].id,"one");
        assert!(page.choices[0].active);
        assert_eq!(page.choices[1].title,"Second task");
        assert_eq!(page.selected,0);
    }

    #[test]
    fn full_terminal_renders_at_multiple_sizes(){
        for (width,height) in [(120,40),(80,24),(45,18),(30,12),(20,10)] {
            let backend=ratatui::backend::TestBackend::new(width,height);
            let mut terminal=ratatui::Terminal::new(backend).unwrap();
            let mut app=App::new("C:/Users/example/projects/webcodex".into());
            app.ready=true;
            app.load_history(&[
                ("user".into(),"A long question that wraps across the terminal.".into()),
                ("assistant".into(),"A response with multiple paragraphs and details.".into()),
            ]);
            app.set_draft("First line\nSecond line\nThird line".into());

            terminal.draw(|frame|render(frame,&mut app)).unwrap();
            assert_eq!(terminal.backend().buffer().area.width,width);
            assert_eq!(terminal.backend().buffer().area.height,height);
        }
    }

    #[test]
    fn header_never_overflows_narrow_terminals(){
        for width in 0..=30 {
            let header=header_lines(
                "C:/Users/example/a/very/long/project",
                width,
                true,
                true
            );
            assert!(header.lines.iter().all(|line|line.width()<=width));
        }
    }

    #[test]
    fn composer_height_remains_bounded_for_huge_pastes(){
        let draft="A very long line\n".repeat(100_000);
        assert_eq!(composer_height(&draft,80,40),12);
        assert_eq!(composer_height(&draft,80,16),8);
    }

    #[test]
    fn composer_visual_rows_match_cursor_wrapping(){
        let draft="abcdef ghij";
        let rows=composer_visual_rows(draft,6);
        assert_eq!(rows,vec!["› abcd","ef ghi","j"]);
        assert_eq!(composer_cursor_position(draft,draft.len(),6),(2,1));

        let multiline="ab\ncd";
        assert_eq!(composer_visual_rows(multiline,5),vec!["› ab","cd"]);
        assert_eq!(composer_cursor_position(multiline,multiline.len(),5),(1,2));
    }

    #[test]
    fn composer_cursor_tracks_explicit_newlines(){
        assert_eq!(composer_cursor_position("",0,40),(0,2));
        assert_eq!(composer_cursor_position("hello",5,40),(0,7));
        assert_eq!(composer_cursor_position("hello\nworld",11,40),(1,5));
        assert_eq!(composer_cursor_position("hello\nworld",6,40),(1,0));
    }

    #[test]
    fn composer_grows_for_multiline_prompts(){
        assert_eq!(composer_height("",100,40),4);
        assert_eq!(composer_height("first\nsecond\nthird",100,40),5);
        assert_eq!(composer_height("one\n\nthree\nfour\nfive",100,40),7);
    }

    #[test]
    fn composer_cursor_tracks_wrapped_text(){
        assert_eq!(composer_cursor_position("abcdef",6,8),(1,0));
        assert_eq!(composer_cursor_position("abcdefg",7,8),(1,1));
        assert_eq!(composer_cursor_position("abcdefghij",10,5),(2,2));
    }

    #[test]
    fn composer_cursor_handles_unicode(){
        let draft="中🙂abc";
        assert_eq!(composer_cursor_position(draft,draft.len(),20),(0,9));
        assert_eq!(composer_cursor_position(draft,draft.len(),6),(1,3));
    }

    #[test]
    fn composer_height_is_bounded(){
        let long_prompt="abcdefghij\n".repeat(100);
        assert_eq!(composer_height(&long_prompt,80,40),12);
        assert!(composer_height(&long_prompt,80,16)<=8);
        assert_eq!(composer_height("hello",80,40),4);
    }

    #[test]
    fn composer_cursor_respects_edit_position(){
        let draft="first\nsecond";
        assert_eq!(composer_cursor_position(draft,3,40),(0,5));
        assert_eq!(composer_cursor_position(draft,6,40),(1,0));
        assert_eq!(composer_cursor_position(draft,draft.len(),40),(1,6));
    }

    #[test]
    fn composer_cursor_handles_non_boundary_offsets(){
        let draft="a🙂b";
        assert_eq!(composer_cursor_position(draft,2,40),(0,3));
        assert_eq!(composer_cursor_position(draft,100,40),(0,6));
    }

    #[test]
    fn composer_accounts_for_wrapped_text(){
        let draft="x".repeat(120);
        assert!(composer_height(&draft,40,40)>4);
        assert!(composer_height(&draft,40,40)<=12);
    }

    #[test]
    fn header_adapts_to_terminal_width(){
        let header=header_lines("C:/Users/example/very/long/project/location",40,false,true);
        assert_eq!(header.lines.len(),2);
        assert!(header.lines[0].width()<=40);
        assert!(header.lines[1].width()<=40);

        let narrow=header_lines("C:/Users/example/project",16,true,true);
        assert!(narrow.lines[1].width()<=16);
        assert!(narrow.lines[0].spans[0].content.contains("WORKING"));
    }

    #[test]
    fn compact_path_preserves_the_project_name(){
        assert_eq!(compact_path("C:/example/webcodex",12),".../webcodex");
        assert_eq!(compact_path("C:/example/webcodex",11),"...webcodex");
        assert_eq!(compact_path("project",20),"project");
        assert_eq!(compact_path("project",0),"");
    }

    #[test]
    fn session_selection_is_bounded(){
        let mut app=App::new("project".into());
        on_message(&mut app,Incoming{
            kind:"page".into(),
            title:String::new(),
            body:r#"{"title":"Saved conversations","lines":[],"choices":[{"id":"first","title":"First","updated":"today","active":false},{"id":"second","title":"Second","updated":"today","active":true}]}"#.into(),
        });
        let page=app.page.as_mut().unwrap();
        assert_eq!(page.selected_session_id(),Some("second"));
        page.select_relative(100);
        assert_eq!(page.selected,1);
        page.select_relative(-100);
        assert_eq!(page.selected_session_id(),Some("first"));
        page.select_relative(1);
        assert_eq!(page.scroll,1);
    }

    #[test]
    fn empty_session_selection_is_safe(){
        let mut app=App::new("project".into());
        on_message(&mut app,Incoming{
            kind:"page".into(),
            title:String::new(),
            body:r#"{"title":"Saved conversations","lines":[],"choices":[]}"#.into(),
        });
        let page=app.page.as_mut().unwrap();
        page.select_relative(100);
        assert!(page.selected_session_id().is_none());
        assert_eq!(page.scroll,0);
    }

    #[test]
    fn page_scrolling_is_bounded(){
        let mut page=PageView{
            title:"Files".into(),
            lines:(0..30).map(|i|format!("File {i}")).collect(),
            choices:vec![],
            selected:0,
            scroll:0,
        };
        page.move_scroll(-10);
        assert_eq!(page.scroll,0);
        page.move_scroll(1000);
        assert_eq!(page.scroll,29);
        page.move_scroll(-1000);
        assert_eq!(page.scroll,0);
    }

    #[test]
    fn settings_panel_selects_real_commands(){
        let mut app=App::new("project".into());
        app.settings=Some(SettingsView{
            auto_apply:false,
            allow_run:false,
            approval_never:false,
            max_turns:100,
        });
        assert_eq!(app.selected_setting_command(),Some("/permissions ask"));
        app.settings_index=1;
        assert_eq!(app.selected_setting_command(),Some("/permissions auto"));
        app.settings_index=2;
        assert_eq!(app.selected_setting_command(),Some("/permissions commands-on"));
        app.settings.as_mut().unwrap().allow_run=true;
        assert_eq!(app.selected_setting_command(),Some("/permissions commands-off"));
        app.settings_index=3;
        assert_eq!(app.selected_setting_command(),Some("/turns 25"));
        app.settings_index=4;
        assert_eq!(app.selected_setting_command(),Some("/turns 100"));
        app.settings_index=5;
        assert_eq!(app.selected_setting_command(),Some("/turns 250"));
        app.settings_index=6;
        assert_eq!(app.selected_setting_command(),Some("/turns 500"));
        app.settings_index=7;
        assert_eq!(app.selected_setting_command(),None);
    }

    #[test]
    fn settings_message_opens_panel(){
        let mut app=App::new("project".into());
        on_message(&mut app,Incoming{
            kind:"settings".into(),
            title:String::new(),
            body:r#"{"auto_apply":false,"allow_run":true,"approval_never":false,"max_turns":100}"#.into(),
        });
        assert!(app.settings.is_some());
        assert_eq!(app.settings.as_ref().unwrap().max_turns,100);
    }

    #[test]
    fn palette_completes_partial_command() {
        let mut app=App::new("project".into());
        app.set_draft("/sta".into());
        assert!(app.complete_palette());
        assert_eq!(app.draft,"/status");
        assert!(!app.complete_palette());
    }

    #[test]
    fn palette_respects_arrow_selection() {
        let mut app=App::new("project".into());
        app.set_draft("/".into());
        let commands=command_palette::search("/");
        app.palette_index=1;
        assert!(app.complete_palette());
        assert_eq!(app.draft,commands[1].name);
        assert_eq!(app.palette_index,0);
    }

    #[test]
    fn palette_resets_selection_after_typing() {
        let mut app=App::new("project".into());
        app.palette_index=4;
        app.put('/');
        assert_eq!(app.palette_index,0);
        app.palette_index=3;
        app.backspace();
        assert_eq!(app.palette_index,0);
    }

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
