#!/usr/bin/env python3
"""Exercise the benchmark runner's timing, metadata, and failure reporting."""
import json
from pathlib import Path
import subprocess
import tempfile
import uuid

root = Path(__file__).resolve().parents[1]
fixture = '''#![allow(non_snake_case)]
#[test] fn passing__ZED_PERF_FN() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}
#[test] fn passing__ZED_PERF_MDATA() { metadata(); }
#[test] fn failing__ZED_PERF_FN() { panic!("fixture failure"); }
#[test] fn failing__ZED_PERF_MDATA() { metadata(); }
fn metadata() {
    println!("ZED_MDATA_version 0\\nZED_MDATA_iter_count 1\\nZED_MDATA_importance average");
}
'''
identifier = 'smoke-' + uuid.uuid4().hex
with tempfile.TemporaryDirectory(prefix='wgpui-perf-') as temporary:
    directory = Path(temporary)
    source = directory / 'fixture.rs'
    binary = directory / 'fixture-tests'
    if __import__('os').name == 'nt':
        binary = binary.with_suffix('.exe')
    source.write_text(fixture)
    subprocess.run(['rustc', '+1.94.0', '--test', str(source), '-o', str(binary)], check=True)
    try:
        subprocess.run(['cargo', '+1.94.0', 'run', '--offline', '-p', 'wgpui-perf', '--',
                        str(binary), '--json=' + identifier], cwd=root, check=True)
        paths = list((root / '.perf-runs').glob(identifier + '.*.json'))
        assert len(paths) == 1, paths
        results = {name: (metadata, result) for name, metadata, result in json.loads(paths[0].read_text())['tests']}
        assert results['passing'][0]['iterations'] == 1
        assert 'Ok' in results['passing'][1], results
        assert results['failing'][1] == {'Err': 'Profile'}, results
        print('PASS performance timing, metadata, and failed-test classification')
    finally:
        for path in (root / '.perf-runs').glob(identifier + '.*.json'):
            path.unlink()
