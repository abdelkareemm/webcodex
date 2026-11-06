import json
import subprocess

worker = "rust-core/target-staging/release/webcodex-core.exe"
request = json.dumps({"op": "parse_reply", "text": "FILE: sample.rs\n"})
result = subprocess.run([worker], input=request + "\n", text=True, capture_output=True)
print(result.stdout)
print(result.stderr)
fence=chr(96)*4
start=chr(60)*7+" SEARCH"
end=chr(62)*7+" REPLACE"
reply=f"{fence}\nFILE: sample.rs\n{start}\nold\n=======\nnew\n\n  indented\n{end}\n{fence}\n"
result=subprocess.run([worker],input=json.dumps({"op":"parse_reply","text":reply})+"\n",text=True,capture_output=True)
parsed=json.loads(result.stdout)
assert result.returncode==0,result.stderr
assert not parsed["errors"],parsed["errors"]
assert parsed["edits"][0]["replace"]=="new\n\n  indented"
print("PASS: multiline edit parsed without errors")
