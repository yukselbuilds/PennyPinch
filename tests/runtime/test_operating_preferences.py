"""The effective tool guard, stats and beacon must be observed, not inferred."""
import json,os,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock
from runtime import desktop_controller as controller, desktop_helper as helper
ROOT=Path(__file__).resolve().parents[2]

class OperatingPreferencesTests(unittest.TestCase):
    def test_safe_child_environment_carries_only_valid_tool_list(self):
        parent=ROOT/'work/proofs/settings/python';parent.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(dir=parent) as temp, mock.patch.dict(os.environ,{'HEADROOM_DESKTOP_PROTECTED_TOOLS':'["ReadExact"]','HEADROOM_PROTECT_TOOL_RESULTS':'inherited-secret','OPENAI_API_KEY':'fixture-secret'}):
            env=controller._safe_child_environment(Path(temp))
            self.assertEqual(json.loads(env['HEADROOM_DESKTOP_PROTECTED_TOOLS']),['ReadExact'])
            self.assertNotIn('HEADROOM_PROTECT_TOOL_RESULTS',env);self.assertNotIn('OPENAI_API_KEY',env)
        with mock.patch.dict(os.environ,{'HEADROOM_DESKTOP_PROTECTED_TOOLS':'["bad name"]'}):
            with self.assertRaises(controller.ProtocolError):controller.protected_tools_from_environment()

    def test_ready_requires_observed_protected_tools_stats_and_beacon(self):
        manifest=json.loads((ROOT/'runtime/manifest.json').read_text(encoding='utf8'));engine=manifest['engine'];shim='a'*64
        health={'config':{'mode':'cache','savings_profile':'coding','optimize':True,'cache':True,'memory':False,'learn':False,'runtime_env':{'HEADROOM_OUTPUT_SHAPER':'0'},'exclude_tools':['ReadExact']}}
        stats={'telemetry':{'enabled':True},'anon_telemetry_shipping':False}
        responses={'/readyz':{'ready':True,'version':engine['version']},'/health':health,'/stats':stats,'/v1/retrieve/stats':{'store':{'default_ttl_seconds':7200}}}
        transport=SimpleNamespace(listener_pids=lambda *_:{'127.0.0.1':99},windows_api=lambda:(None,None,None),request_json=lambda _,path:(200,responses[path]))
        process=SimpleNamespace(pid=99,poll=lambda:None)
        output=SimpleNamespace(wait_for_start_markers=lambda:True,shim_marker=(engine['version'],engine['prefix_tracker_sha256'],shim),control_marker=(manifest['optimization_control']['sha256'],manifest['optimization_control']['sources_sha256']))
        with mock.patch.dict(os.environ,{'HEADROOM_DESKTOP_PROTECTED_TOOLS':'["ReadExact"]'}):
            effective,_=helper._verify_live_runtime(transport,process,1234,output,manifest,shim)
            self.assertEqual(effective['protected_tools'],['ReadExact']);self.assertTrue(effective['telemetry_enabled']);self.assertFalse(effective['beacon_enabled'])
            health['config']['exclude_tools']=[]
            with self.assertRaisesRegex(RuntimeError,'protected-tools'):helper._verify_live_runtime(transport,process,1234,output,manifest,shim)
            health['config']['exclude_tools']=['ReadExact'];stats['anon_telemetry_shipping']=True
            with self.assertRaisesRegex(RuntimeError,'telemetry'):helper._verify_live_runtime(transport,process,1234,output,manifest,shim)

if __name__=='__main__':unittest.main()
