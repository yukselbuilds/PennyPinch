"""Owned deterministic command fixtures. No user repository, account or provider is used."""
import json
from pathlib import Path
import subprocess
import sys
import unittest

EXPECTED_EXIT = {"git_status": 0, "git_diff": 0, "test_pass": 0, "test_fail": 1,
                 "build_fail": 2, "json_exact": 0, "source_read": 0, "stderr_exit": 23}


def create(workspace, environment, git):
    workspace.mkdir()
    (workspace / "source.py").write_bytes(
        ("# Türkçe λ: preserve source and exact numbers\r\n" + "\r\n".join(
            f"def value_{n}():\r\n    return '9007199254740993:0.000000000000001:{n:04d}'"
            for n in range(80)) + "\r\n").encode("utf-8"))
    numeric = '[\n' + ',\n'.join(
        '  {"index":%d,"integer":18446744073709551617,"decimal":0.1234567890123456789012345,"negative":-17.2500}' % n
        for n in range(40)) + '\n]\n'
    (workspace / "numbers.json").write_bytes(numeric.encode("utf-8"))
    source = "\n".join(f"pub const VALUE_{n}: u64 = {9007199254740993 + n};" for n in range(100)) + "\n"
    (workspace / "values.rs").write_text(source, encoding="utf-8", newline="\n")
    (workspace / "identity.txt").write_text("owned local fixture\n", encoding="utf-8")
    def execute(*args):
        subprocess.run([str(git), *args], cwd=workspace, env=environment, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    execute("init", "--quiet")
    execute("config", "user.name", "Headroom local fixture")
    execute("config", "user.email", "fixture@localhost.invalid")
    execute("config", "core.autocrlf", "false")
    execute("config", "core.hooksPath", str(workspace / "empty-hooks"))
    execute("add", "values.rs", "identity.txt")
    execute("commit", "--quiet", "-m", "Synthetic fixture baseline")
    (workspace / "values.rs").write_text(source.replace("9007199254740993", "9007199254740999"), encoding="utf-8", newline="\n")
    for n in range(100):
        (workspace / f"untracked_{n:03d}.txt").write_text(f"fixture {n}\n", encoding="utf-8")


def commands(case, workspace, git, rtk):
    python = [sys.executable, "-I", "-B", str(Path(__file__).resolve())]
    if case == "git_status":
        command = [str(git), "status"]
        filtered = [str(rtk), "git", "status"]
    elif case == "git_diff":
        command = [str(git), "diff", "--no-ext-diff", "--no-color"]
        filtered = [str(rtk), "git", *command[1:]]
    elif case in ("json_exact", "source_read"):
        name = "numbers.json" if case == "json_exact" else "source.py"
        command = python + ["--cat", str(workspace / name)]
        filtered = [str(rtk), "json" if case == "json_exact" else "read", str(workspace / name)]
    else:
        command = python + ["--emit", case]
        filtered = [str(rtk), "err" if case in ("build_fail", "stderr_exit") else "test", *command]
    return command, filtered


class FixtureResult(unittest.TestResult):
    def addSuccess(self, test):
        super().addSuccess(test)
        print("test " + test.id().rsplit(".", 1)[-1] + " ... ok")

    def addFailure(self, test, error):
        super().addFailure(test, error)
        print("test " + test.id().rsplit(".", 1)[-1] + " ... FAILED")
        sys.stderr.write("FAIL: EXACT_FAILURE_9007199254740993 expected=-17.2500 actual=0.000000000000001\n")


def emit(case):
    if case in ("test_pass", "test_fail"):
        methods = {f"test_exact_{n:03d}": (lambda self: self.assertEqual(2 + 2, 4)) for n in range(100)}
        if case == "test_fail":
            methods["test_exact_failure"] = lambda self: self.assertEqual(2 + 2, 5)
        tests = unittest.defaultTestLoader.loadTestsFromTestCase(type("OwnedTests", (unittest.TestCase,), methods))
        result = FixtureResult()
        tests.run(result)
        print(f"test result: {'ok' if result.wasSuccessful() else 'FAILED'}. {result.testsRun-len(result.failures)} passed; {len(result.failures)} failed; 0 ignored")
        return 0 if result.wasSuccessful() else 1
    if case == "build_fail":
        for n in range(100):
            print(f"Compiling fixture_{n:03d} v0.0.0")
        try:
            compile("def missing_close(:\n    pass\n", "owned_build_fixture.py", "exec")
        except SyntaxError as error:
            sys.stderr.write(f"error: {type(error).__name__}: {error.msg} at owned_build_fixture.py:{error.lineno}:{error.offset}\n")
            return 2
        raise AssertionError("invalid_fixture_compiled")
    if case == "stderr_exit":
        for n in range(150):
            print(f"INFO fixture request completed sequence={n:04d} exact=9007199254740993 latency=17.2500")
        sys.stderr.write("WARNING: UNIQUE_STDERR_9007199254740993 value=-17.2500\n")
        return 23
    raise AssertionError("unknown_fixture")


def body(text, case, iteration, round_number):
    tool = "Read" if case in ("json_exact", "source_read", "git_diff") else "inspect_workspace"
    return {
        "model": "o4-mini", "reasoning": {"effort": "high"}, "stream": True,
        "tools": [{"type": "function", "name": tool, "description": "Read synthetic command result",
                   "parameters": {"type": "object", "properties": {}}}],
        "input": [
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Inspect the local command result."}]},
            {"type": "function_call", "call_id": f"owned-{iteration}-{case}-{round_number}", "name": tool, "arguments": "{}"},
            {"type": "function_call_output", "call_id": f"owned-{iteration}-{case}-{round_number}", "output": text},
        ],
    }


if __name__ == "__main__":
    if sys.argv[1:2] == ["--cat"] and len(sys.argv) == 3:
        sys.stdout.buffer.write(Path(sys.argv[2]).read_bytes())
    elif sys.argv[1:2] == ["--emit"] and len(sys.argv) == 3:
        sys.stdout.reconfigure(encoding="utf-8", newline="\n")
        sys.stderr.reconfigure(encoding="utf-8", newline="\n")
        raise SystemExit(emit(sys.argv[2]))
    else:
        raise SystemExit("explicit_owned_fixture_required")
