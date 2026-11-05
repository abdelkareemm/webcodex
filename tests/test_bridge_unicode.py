"""Exercise the bridge with a restrictive Windows-style output encoding."""
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

ROOT=Path(__file__).resolve().parents[1]

class UnicodeBridgeTests(unittest.TestCase):
    def test_legacy_charmap_does_not_crash_diagnostics_or_json(self):
        script='''
import json, sys
from webcodex.browser_bridge import configure_pipe_encoding, emit
configure_pipe_encoding()
print('Working 👋',file=sys.stderr)
emit({'kind':'update','text':'Hello 👋'})
'''
        env=os.environ.copy();env['PYTHONIOENCODING']='cp1252:strict'
        env['PYTHONPATH']=str(ROOT)
        p=subprocess.run([sys.executable,'-c',script],capture_output=True,env=env,cwd=ROOT,timeout=10)
        self.assertEqual(p.returncode,0,p.stderr.decode('utf-8','replace'))
        self.assertEqual(json.loads(p.stdout.decode('utf-8'))['text'],'Hello 👋')
        self.assertIn('👋',p.stderr.decode('utf-8'))

if __name__=='__main__':unittest.main()
