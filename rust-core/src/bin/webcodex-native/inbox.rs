use std::{fs,path::{Path,PathBuf}};

pub struct Inbox {
    path:PathBuf,
    items:Vec<String>,
    load_error:Option<String>,
}

impl Inbox {
    pub fn open(root:&Path)->Self {
        let path=root.join(".webcodex/queue.json");
        let (items,load_error)=match fs::read(&path){
            Ok(data)=>match serde_json::from_slice::<Vec<String>>(&data){
                Ok(items)=>(items,None),
                Err(e)=>(Vec::new(),Some(format!("Invalid queue.json: {e}. Restore or repair the file before continuing."))),
            },
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>(Vec::new(),None),
            Err(e)=>(Vec::new(),Some(format!("Cannot read queue.json: {e}"))),
        };
        Self{path,items,load_error}
    }

    fn save(&self)->Result<(),String>{
        if let Some(error)=&self.load_error{return Err(error.clone())}
        if let Some(parent)=self.path.parent(){
            fs::create_dir_all(parent).map_err(|e|e.to_string())?;
        }
        let data=serde_json::to_vec_pretty(&self.items).map_err(|e|e.to_string())?;
        let temporary=self.path.with_extension("json.tmp");
        let result=(|| -> Result<(),String> {
            use std::io::Write;
            let mut file=fs::File::create(&temporary).map_err(|e|e.to_string())?;
            file.write_all(&data).map_err(|e|e.to_string())?;
            file.sync_all().map_err(|e|e.to_string())?;
            drop(file);
            fs::rename(&temporary,&self.path).map_err(|e|e.to_string())
        })();
        if result.is_err(){
            let _=fs::remove_file(&temporary);
        }
        result
    }

    pub fn push(&mut self,message:String)->Result<usize,String>{
        if message.trim().is_empty(){return Err("Empty message".into())}
        if self.items.len()>=1000{return Err("Queue limit reached".into())}
        self.items.push(message);
        if let Err(error)=self.save(){
            self.items.pop();
            return Err(error);
        }
        Ok(self.items.len())
    }

    pub fn pop(&mut self)->Result<Option<String>,String>{
        if let Some(error)=&self.load_error{return Err(error.clone())}
        if self.items.is_empty(){return Ok(None)}
        let inflight=self.path.with_file_name("queue-inflight.json");
        if inflight.exists(){
            return Err("An interrupted queued task requires /queue recover or /queue discard".into());
        }
        let item=self.items[0].clone();
        let data=serde_json::to_vec(&item).map_err(|e|e.to_string())?;
        {
            use std::io::Write;
            let mut file=fs::OpenOptions::new()
                .write(true).create_new(true).open(&inflight)
                .map_err(|e|format!("Cannot create recovery record: {e}"))?;
            let result=file.write_all(&data).and_then(|_|file.sync_all());
            drop(file);
            if let Err(e)=result {
                let _=fs::remove_file(&inflight);
                return Err(format!("Recovery record write failed: {e}"));
            }
        }
        self.items.remove(0);
        if let Err(error)=self.save(){
            self.items.insert(0,item);
            let _=fs::remove_file(&inflight);
            return Err(error);
        }
        Ok(Some(item))
    }

    pub fn acknowledge(&mut self)->Result<(),String>{
        let path=self.path.with_file_name("queue-inflight.json");
        if path.exists(){
            fs::remove_file(path).map_err(|e|e.to_string())?;
        }
        Ok(())
    }

    pub fn manage(&mut self,command:&str)->Result<String,String>{
        if command.trim()=="/queue"{return Ok(self.show())}
        if let Some(error)=&self.load_error{return Err(error.clone())}
        let previous=self.items.clone();
        let args:Vec<&str>=command.split_whitespace().collect();
        match args.as_slice(){
            ["/queue"]=>return Ok(self.show()),
            ["/queue","clear"]=>self.items.clear(),
            ["/queue","recover"]=>{
                let path=self.path.with_file_name("queue-inflight.json");
                let data=fs::read(&path).map_err(|e|e.to_string())?;
                let message:String=serde_json::from_slice(&data).map_err(|e|e.to_string())?;
                if self.items.first()==Some(&message){
                    self.acknowledge()?;
                    return Ok("Interrupted message is already first in the queue.".into());
                }
                self.items.insert(0,message);
                if let Err(error)=self.save(){
                    self.items.remove(0);
                    return Err(error);
                }
                self.acknowledge()?;
                return Ok(format!("Restored interrupted message.\n{}",self.show()));
            },
            ["/queue","discard"]=>{
                self.acknowledge()?;
                return Ok("Interrupted-message recovery record discarded.".into());
            },
            ["/queue","remove",n]=>{
                let n=n.parse::<usize>().map_err(|_|"Invalid index")?;
                if n==0||n>self.items.len(){return Err("Index out of range".into())}
                self.items.remove(n-1);
            },
            ["/queue","move",from,to]=>{
                let from=from.parse::<usize>().map_err(|_|"Invalid source index")?;
                let to=to.parse::<usize>().map_err(|_|"Invalid destination index")?;
                if from==0||to==0||from>self.items.len()||to>self.items.len(){
                    return Err("Index out of range".into())
                }
                let item=self.items.remove(from-1);
                self.items.insert(to-1,item);
            },
            _ if command.starts_with("/queue edit ")=>{
                let rest=command.trim_start_matches("/queue edit ").trim();
                let (position,replacement)=rest.split_once(' ')
                    .ok_or("Usage: /queue edit N <new message>")?;
                let position=position.parse::<usize>().map_err(|_|"Invalid index")?;
                if position==0||position>self.items.len(){
                    return Err("Index out of range".into())
                }
                if replacement.trim().is_empty(){return Err("Empty replacement".into())}
                self.items[position-1]=replacement.trim().to_string();
            },
            _=>return Err("Usage: /queue [clear|remove N|move N N|edit N text|recover|discard]".into())
        }
        if let Err(error)=self.save(){
            self.items=previous;
            return Err(error);
        }
        Ok(self.show())
    }

    pub fn has_load_error(&self)->bool{
        self.load_error.is_some()
    }

    pub fn has_inflight(&self)->bool{
        self.path.with_file_name("queue-inflight.json").exists()
    }

    pub fn len(&self)->usize{
        self.items.len()
    }

    pub fn show(&self)->String{
        if let Some(error)=&self.load_error{
            return format!("Queue unavailable: {error}");
        }
        if self.items.is_empty(){
            return if self.has_inflight(){
                "No pending messages. An interrupted task requires /queue recover or /queue discard.".into()
            }else{
                "Queue is empty.".into()
            };
        }
        let mut lines=self.items.iter().enumerate().take(50).map(|(i,item)|{
            format!("{}. {}",i+1,item.chars().take(120).collect::<String>())
        }).collect::<Vec<_>>();
        if self.has_inflight(){
            lines.insert(0,"Interrupted task awaiting recovery. /queue recover may resend the message; /queue discard removes its recovery record.".into());
        }
        if let Some(error)=&self.load_error{
            lines.insert(0,error.clone());
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_save_preserves_pending_messages(){
        let root=std::env::temp_dir().join(format!("webcodex-queue-failure-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("keep me".into()).unwrap();

        let blocker=root.join("blocker");
        fs::write(&blocker,b"not a directory").unwrap();
        queue.path=blocker.join("queue.json");

        assert!(queue.manage("/queue clear").is_err());
        assert_eq!(queue.show(),"1. keep me");

        assert!(queue.pop().is_err());
        assert_eq!(queue.show(),"1. keep me");

        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn existing_recovery_record_is_preserved(){
        let root=std::env::temp_dir().join(format!("wc-inflight-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("first".into()).unwrap();
        queue.push("second".into()).unwrap();
        assert_eq!(queue.pop().unwrap().as_deref(),Some("first"));

        let record=root.join(".webcodex/queue-inflight.json");
        let original=fs::read(&record).unwrap();
        assert!(queue.pop().is_err());
        assert_eq!(fs::read(&record).unwrap(),original);
        assert_eq!(queue.len(),1);

        queue.manage("/queue recover").unwrap();
        assert_eq!(queue.len(),2);
        assert_eq!(queue.pop().unwrap().as_deref(),Some("first"));
        queue.acknowledge().unwrap();
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_task_requires_manual_recovery(){
        let root=std::env::temp_dir().join(format!("webcodex-recovery-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("first".into()).unwrap();
        queue.push("second".into()).unwrap();
        assert_eq!(queue.pop().unwrap().as_deref(),Some("first"));
        assert!(queue.has_inflight());
        assert!(queue.pop().is_err());
        drop(queue);
        let mut restarted=Inbox::open(&root);
        restarted.manage("/queue recover").unwrap();
        assert_eq!(restarted.pop().unwrap().as_deref(),Some("first"));
        restarted.acknowledge().unwrap();
        assert_eq!(restarted.pop().unwrap().as_deref(),Some("second"));
        restarted.acknowledge().unwrap();
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn recovering_an_already_queued_message_does_not_duplicate_it(){
        let root=std::env::temp_dir().join(format!("webcodex-dedup-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("first".into()).unwrap();
        queue.push("second".into()).unwrap();
        assert_eq!(queue.pop().unwrap().as_deref(),Some("first"));
        queue.items.insert(0,"first".into());
        queue.save().unwrap();
        queue.manage("/queue recover").unwrap();
        assert_eq!(queue.len(),2);
        assert!(!queue.has_inflight());
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn queue_supports_edit_move_and_remove(){
        let root=std::env::temp_dir().join(format!("webcodex-queue-edit-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("alpha".into()).unwrap();
        queue.push("beta".into()).unwrap();
        queue.push("gamma".into()).unwrap();
        queue.manage("/queue edit 2 revised").unwrap();
        queue.manage("/queue move 3 1").unwrap();
        assert_eq!(Inbox::open(&root).show(),"1. gamma\n2. alpha\n3. revised");
        queue.manage("/queue remove 2").unwrap();
        assert_eq!(queue.pop().unwrap().as_deref(),Some("gamma"));
        queue.acknowledge().unwrap();
        assert_eq!(queue.pop().unwrap().as_deref(),Some("revised"));
        queue.acknowledge().unwrap();
        assert_eq!(queue.pop().unwrap(),None);
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_writes_replace_existing_queue_file(){
        let root=std::env::temp_dir().join(format!("webcodex-replace-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("first".into()).unwrap();
        queue.push("second".into()).unwrap();
        queue.manage("/queue edit 1 changed").unwrap();
        assert_eq!(Inbox::open(&root).show(),"1. changed\n2. second");
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_queue_is_not_overwritten(){
        let root=std::env::temp_dir().join(format!("webcodex-corrupt-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".webcodex")).unwrap();
        let path=root.join(".webcodex/queue.json");
        fs::write(&path,b"{invalid").unwrap();
        let mut queue=Inbox::open(&root);
        assert!(queue.push("new task".into()).is_err());
        assert!(queue.pop().is_err());
        assert_eq!(fs::read(&path).unwrap(),b"{invalid");
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_queue_reports_error_without_overwriting(){
        let root=std::env::temp_dir().join(format!("webcodex-corrupt-display-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".webcodex")).unwrap();
        let path=root.join(".webcodex/queue.json");
        fs::write(&path,b"invalid json").unwrap();

        let mut queue=Inbox::open(&root);
        assert!(queue.show().contains("Queue unavailable"));
        assert!(queue.push("new task".into()).is_err());
        assert_eq!(fs::read(&path).unwrap(),b"invalid json");

        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_message_survives_restart_until_acknowledged(){
        let root=std::env::temp_dir().join(format!("webcodex-restart-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut queue=Inbox::open(&root);
        queue.push("finish this".into()).unwrap();
        assert_eq!(queue.pop().unwrap().as_deref(),Some("finish this"));
        drop(queue);

        let mut restarted=Inbox::open(&root);
        assert!(restarted.has_inflight());
        assert_eq!(restarted.len(),0);
        restarted.manage("/queue discard").unwrap();
        assert!(!restarted.has_inflight());

        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn queue_is_persistent_and_ordered(){
        let root=std::env::temp_dir().join(format!("webcodex-queue-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let mut q=Inbox::open(&root);
        q.push("first".into()).unwrap();
        q.push("second".into()).unwrap();
        assert_eq!(Inbox::open(&root).show(),"1. first\n2. second");
        assert_eq!(q.pop().unwrap().as_deref(),Some("first"));
        let _=fs::remove_dir_all(root);
    }
}
