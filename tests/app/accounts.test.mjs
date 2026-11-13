import test from "node:test";
import assert from "node:assert/strict";
import { createAccountClient, decodeAccountReply } from "../../src/lib/accounts.ts";

const id = "e7358031-a684-44fd-84e1-3b2577712578";
const requestId = "9dca9984-2b7d-4b8a-b3c0-a77c44f6a810";
const flowId = "1333248d-a929-45e6-81f8-c90173aa5aad";
const installation = { installation_id: id, client_kind: "codex", surface: "cli", version: "0.163.0-alpha.11.2", source: "manual_file", location_label: "selected_file", status: "detected", connection_status: "not_verified" };
const account = { installation_id: id, client_kind: "codex", surface: "cli", state: "present", method: "chatgpt", source: "official_cli", reason: "none", login_supported: true, remote_verified: false, providers: [] };
const request = { schema_version: 1, request_id: requestId, operation: "read", installation_id: id, scan_generation: 1, flow_id: null, confirm_profile_change: false };
const reply = a => ({ schema_version: 1, request_id: requestId, status: "snapshot", account: a, flow: null, error: null });

test("account metadata is correlated to the selected installation and remains locally scoped", () => {
  assert.equal(decodeAccountReply(reply(account), request, installation).kind, "ok");
  for (const patch of [{remote_verified: true}, {token: "private-sentinel"}, {installation_id: flowId}, {client_kind: "opencode"}, {surface: "desktop"}, {state: "absent"}]) {
    assert.equal(decodeAccountReply(reply({...account, ...patch}), request, installation).kind, "error");
  }
  assert.equal(decodeAccountReply({...reply(account), request_id: flowId}, request, installation).kind, "error");
});

test("unknown and stored provider metadata do not become verified accounts", () => {
  const unknown = {...account, state: "unknown", method: "unknown", source: "unavailable", reason: "read_not_passive"};
  assert.equal(decodeAccountReply(reply(unknown), request, installation).kind, "ok");
  const opencode = {...installation, client_kind: "opencode"};
  const stored = {...account, client_kind: "opencode", method: "provider_credentials", source: "local_store", providers: [{provider:"openai",method:"api_key"}]};
  assert.equal(decodeAccountReply(reply(stored), request, opencode).kind, "ok");
  stored.providers = [{provider:"private-sentinel",method:"api_key"}];
  assert.equal(decodeAccountReply(reply(stored), request, opencode).kind, "error");
});

test("login never invokes without explicit scope consent and request contains no executable or model", async () => {
  const calls = [];
  const client = createAccountClient(() => true, async (command,args) => {
    calls.push({command,args});
    return { ...reply(null), flow: {flow_id:flowId, installation_id:id, client_kind:"codex",state:"pending"} };
  }, () => requestId);
  assert.equal((await client.start(installation,1,false)).kind, "error"); assert.equal(calls.length,0);
  assert.equal((await client.start({...installation, executable_path:"private-sentinel", model:"private-model"},1,true)).kind,"ok");
  assert.equal(calls[0].command,"manage_account");
  assert.equal(calls[0].args.request.confirm_profile_change,true);
  assert.ok(!JSON.stringify(calls).includes("private-"));
});

test("owned flow refresh and cancellation reject another process identity", async () => {
  const flow = {flow_id:flowId, installation_id:id, client_kind:"codex",state:"pending"};
  const client = createAccountClient(() => true, async (_command,args) => ({ ...reply(null), flow: {...flow, flow_id:requestId, state:"cancelled"} }), () => requestId);
  assert.equal((await client.cancel(flow)).kind,"error");
  const active = createAccountClient(() => true, async () => reply(null), () => requestId);
  assert.deepEqual(await active.active(),{kind:"ok",account:null,flow:null});
});

test("preview, bad requests and transport errors never expose exception content", async () => {
  let count=0;
  const unavailable=createAccountClient(()=>false,async()=>{count++;throw Error("private-sentinel")});
  assert.deepEqual(await unavailable.read(installation,1),{kind:"error",code:"desktop_unavailable"});
  assert.equal(count,0);
  const client=createAccountClient(()=>true,async()=>{count++;throw Error("private-sentinel")},()=>requestId);
  assert.equal((await client.read(installation,0)).kind,"error"); assert.equal(count,0);
  assert.deepEqual(await client.read(installation,1),{kind:"error",code:"unavailable"});
});
