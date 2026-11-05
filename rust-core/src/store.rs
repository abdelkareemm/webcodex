//! SQLite schema and storage compatible with the historical WebCodex Python store.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

pub struct Store { db: Connection }
fn timestamp() -> String { chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true) }
const TITLE: &str = "New WebCodex thread";
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        let db=Connection::open(path).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS threads (id TEXT PRIMARY KEY, title TEXT NOT NULL, project_root TEXT, chat_url TEXT, state TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY AUTOINCREMENT, thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE, role TEXT NOT NULL CHECK(role IN ('system','user','assistant')), content TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id, id);").map_err(|e| e.to_string())?;
        let mut stmt=db.prepare("PRAGMA table_info(threads)").map_err(|e|e.to_string())?;
        let columns:Vec<String>=stmt.query_map([],|r|r.get(1)).map_err(|e|e.to_string())?.filter_map(Result::ok).collect();
        drop(stmt);
        for col in ["chat_url","state"] { if !columns.iter().any(|s|s==col) {
            db.execute(&format!("ALTER TABLE threads ADD COLUMN {col} TEXT"),[]).map_err(|e|e.to_string())?;
        }}
        Ok(Self {db})
    }
    fn row(row:&rusqlite::Row<'_>)->rusqlite::Result<Value> {
        let keys=["id","title","project_root","chat_url","state","created_at","updated_at"];
        let mut map=serde_json::Map::new();
        for key in keys { if let Ok(s)=row.get::<_,Option<String>>(key) { map.insert(key.into(),s.map(Value::String).unwrap_or(Value::Null)); } }
        Ok(Value::Object(map))
    }
    pub fn handle(&mut self, op:&str, req:&Value)->Result<Value,String> {
        let s=|name:&str|req.get(name).and_then(Value::as_str).unwrap_or("");
        match op {
            "store_create" => {
                let id=Uuid::new_v4().simple().to_string()[..16].to_string(); let now=timestamp();
                let title=s("title");let title=if title.is_empty(){TITLE}else{title};
                let title:String=title.chars().take(120).collect();
                self.db.execute("INSERT INTO threads(id,title,project_root,chat_url,state,created_at,updated_at) VALUES(?1,?2,?3,?4,NULL,?5,?5)",params![id,title,s("project_root"),req.get("chat_url").and_then(Value::as_str),now]).map_err(|e|e.to_string())?;
                Ok(json!({"id":id}))
            }
            "store_get"=>{
                let v=self.db.query_row("SELECT * FROM threads WHERE id=?1",[s("thread_id")],Self::row).optional().map_err(|e|e.to_string())?;
                Ok(json!({"thread":v}))
            }
            "store_find_url"=>{
                let v=self.db.query_row("SELECT * FROM threads WHERE chat_url=?1 AND project_root=?2 ORDER BY updated_at DESC LIMIT 1",
                    params![s("url"),s("project_root")],Self::row).optional().map_err(|e|e.to_string())?;
                Ok(json!({"thread":v}))
            }
            "store_latest"=>{
                let v=self.db.query_row("SELECT * FROM threads WHERE project_root=?1 ORDER BY updated_at DESC LIMIT 1",[s("project_root")],Self::row).optional().map_err(|e|e.to_string())?;
                Ok(json!({"thread":v}))
            }
            "store_list"=>{
                let limit=req.get("limit").and_then(Value::as_i64).unwrap_or(30).clamp(1,10000);
                let project=req.get("project_root").and_then(Value::as_str).filter(|v|!v.is_empty());
                let query=if project.is_some(){"SELECT * FROM threads WHERE project_root=?1 ORDER BY updated_at DESC LIMIT ?2"}else{"SELECT * FROM threads ORDER BY updated_at DESC LIMIT ?1"};
                let mut st=self.db.prepare(query).map_err(|e|e.to_string())?;
                let rows=if let Some(project)=project {st.query_map(params![project,limit],Self::row)} else {st.query_map(params![limit],Self::row)}.map_err(|e|e.to_string())?;
                Ok(json!({"threads":rows.filter_map(Result::ok).collect::<Vec<_>>()}))
            }
            "store_set_url"=>{self.db.execute("UPDATE threads SET chat_url=?1,updated_at=?2 WHERE id=?3",params![s("url"),timestamp(),s("thread_id")]).map_err(|e|e.to_string())?;Ok(json!({"ok":true}))}
            "store_save_state"=>{self.db.execute("UPDATE threads SET state=?1,updated_at=?2 WHERE id=?3",params![req.get("state").unwrap_or(&Value::Null).to_string(),timestamp(),s("thread_id")]).map_err(|e|e.to_string())?;Ok(json!({"ok":true}))}
            "store_load_state"=>{
                let raw:Option<String>=self.db.query_row("SELECT state FROM threads WHERE id=?1",[s("thread_id")],|r|r.get(0)).optional().map_err(|e|e.to_string())?.flatten();
                let state:Value=raw.and_then(|v|serde_json::from_str(&v).ok()).filter(Value::is_object).unwrap_or_else(||json!({}));
                Ok(json!({"state":state}))
            }
            "store_add_message"=>{
                let role=s("role");if !["system","user","assistant"].contains(&role){return Err("invalid role".into())};
                let now=timestamp();let id=s("thread_id");let content=s("content");
                self.db.execute("INSERT INTO messages(thread_id,role,content,created_at) VALUES(?1,?2,?3,?4)",params![id,role,content,now]).map_err(|e|e.to_string())?;
                if role=="user"&&!content.trim().is_empty(){
                    let title:String=content.trim().lines().next().unwrap_or("").chars().take(100).collect();
                    self.db.execute("UPDATE threads SET title=?1,updated_at=?2 WHERE id=?3 AND title=?4",params![title,now,id,TITLE]).map_err(|e|e.to_string())?;
                }
                self.db.execute("UPDATE threads SET updated_at=?1 WHERE id=?2",params![now,id]).map_err(|e|e.to_string())?;
                Ok(json!({"ok":true}))
            }
            "store_messages"=>{
                let mut st=self.db.prepare("SELECT role,content FROM messages WHERE thread_id=?1 ORDER BY id DESC LIMIT ?2").map_err(|e|e.to_string())?;
                let mut m:Vec<Value>=st.query_map(params![s("thread_id"),req.get("limit").and_then(Value::as_i64).unwrap_or(40)],|r|Ok(json!({"role":r.get::<_,String>(0)?,"content":r.get::<_,String>(1)?}))).map_err(|e|e.to_string())?.filter_map(Result::ok).collect();
                m.reverse();Ok(json!({"messages":m}))
            }
            _=>Err(format!("unknown store operation: {op}"))
        }
    }
}
