//! Small, Codex-like arrow-key session selector before starting the browser.
use crossterm::{cursor,execute,event::{self,Event,KeyCode,KeyEventKind,KeyModifiers},terminal::{self,EnterAlternateScreen,LeaveAlternateScreen,Clear,ClearType}};
use std::{io::{self,Write,IsTerminal},time::Duration};

struct Screen;
impl Drop for Screen {fn drop(&mut self){let _=terminal::disable_raw_mode();let _=execute!(io::stdout(),LeaveAlternateScreen,cursor::Show);}}

pub fn pick(title:&str,items:&[(String,String)],allow_new:bool)->Result<Option<usize>,String>{
    if items.is_empty(){return Ok(None)}
    if !io::stdin().is_terminal() || !io::stdout().is_terminal(){
        if allow_new{return Ok(Some(0))}
        return Err("Interactive Codex import requires a terminal; specify --session <id> in scripts".into());
    }
    terminal::enable_raw_mode().map_err(|e|e.to_string())?;
    let _screen=Screen;
    execute!(io::stdout(),EnterAlternateScreen,cursor::Hide).map_err(|e|e.to_string())?;
    let mut selected=0usize;let mut filter=String::new();
    loop {
        let (width,height)=terminal::size().unwrap_or((100,30));
        let max_visible=(height as usize).saturating_sub(7).max(1);
        let matching:Vec<usize>=items.iter().enumerate().filter(|(_,(_,label))|filter.is_empty()||label.to_lowercase().contains(&filter.to_lowercase())).map(|(i,_)|i).collect();
        if selected>=matching.len(){selected=matching.len().saturating_sub(1)}
        let start=selected.saturating_sub(max_visible/2).min(matching.len().saturating_sub(max_visible));
        let mut out=io::stdout();
        execute!(out,cursor::MoveTo(0,0),Clear(ClearType::All)).map_err(|e|e.to_string())?;
        println!("  WebCodex · {title}\n");
        println!("  Search: {filter}{}",if filter.is_empty(){"(type to filter)"}else{""});
        println!("  ↑↓ select  ·  Enter open  ·  Esc cancel  ·  Backspace edit\n");
        for (idx,&item) in matching.iter().enumerate().skip(start).take(max_visible){
            let marker=if idx==selected {"❯"}else{" "};
            let label=&items[item].1;
            let max_chars=(width as usize).saturating_sub(7);
            let display:String=label.chars().take(max_chars).collect();
            println!("  {marker} {display}");
        }
        if matching.is_empty(){println!("  No matching sessions")}
        if allow_new {println!("\n  Esc starts a new session")}else{println!("\n  Esc cancels import")}
        out.flush().map_err(|e|e.to_string())?;
        if !event::poll(Duration::from_millis(250)).map_err(|e|e.to_string())? {continue}
        if let Event::Key(key)=event::read().map_err(|e|e.to_string())?{
            if key.kind!=KeyEventKind::Press {continue}
            match key.code {
                KeyCode::Up=>selected=selected.saturating_sub(1),
                KeyCode::Down=>selected=(selected+1).min(matching.len().saturating_sub(1)),
                KeyCode::Enter=>return Ok(matching.get(selected).copied()),
                KeyCode::Esc=>return Ok(None),
                KeyCode::Backspace=>{filter.pop();selected=0},
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL)=>return Ok(None),
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)&&!key.modifiers.contains(KeyModifiers::ALT)=>{filter.push(c);selected=0},
                _=>{}
            }
        }
    }
}
