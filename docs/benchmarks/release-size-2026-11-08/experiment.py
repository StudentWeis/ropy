"""Disposable experiments only. Never modifies the checkout or registry cache."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import time

REPO = Path.cwd()
ROOT = REPO / "target/size-speed/final"
ROOT.mkdir(exist_ok=False)
REGISTRY = Path.home() / ".cargo/registry/src/index.crates.io-1949cf8c6b5b557f"
REVISION = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
(ROOT / "revision.txt").write_text(REVISION + "\n")
with (ROOT / "source.tar").open("wb") as output:
    subprocess.run(["git", "archive", "HEAD"], stdout=output, check=True)
BASE_ENV = dict(os.environ, CARGO_BUILD_JOBS="4")
for key in list(BASE_ENV):
    if key.startswith("CARGO_PROFILE_"):
        del BASE_ENV[key]


def run(args, cwd, log, env):
    started = time.monotonic()
    with log.open("w") as output:
        result = subprocess.run(args, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode:
        print(log.read_text()[-9000:], flush=True)
        raise RuntimeError(f"{args}: exit {result.returncode}; log={log}")
    return time.monotonic() - started


def replace_function(source, signature, body):
    start = source.index(signature)
    opening = source.index("{", start)
    # Top-level impl methods close at four spaces; free functions at column zero.
    closing = source.index("\n    }" if signature.startswith("    ") else "\n}", opening)
    return source[:opening + 1] + "\n" + body + source[closing:]


def drop_dependency(source, name):
    lines = source.splitlines(keepends=True)
    start = next(i for i, line in enumerate(lines) if line.startswith("[") and line.rstrip().endswith(f"dependencies.{name}]"))
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("[")), len(lines))
    return "".join(lines[:start] + lines[end:])


def prepare(name, capabilities=(), codecs=False, mixed=False):
    work = ROOT / name
    work.mkdir()
    with tarfile.open(ROOT / "source.tar") as archive:
        archive.extractall(work, filter="data")
    manifest = work / "Cargo.toml"
    patches = []
    if capabilities:
        package = "gpui-pre-linux"
        vendor = work / "experiment-vendor" / package
        shutil.copytree(REGISTRY / f"{package}-0.3.11", vendor)
        patches.append((package, vendor))
        cargo = (vendor / "Cargo.toml").read_text()
        platform = vendor / "src/linux/platform.rs"
        source = platform.read_text()
        if "credentials" in capabilities:
            cargo = drop_dependency(cargo, "oo7")
            for method in ("write_credentials", "read_credentials", "delete_credentials"):
                source = replace_function(source, f"    fn {method}(", '        Task::ready(Err(anyhow!("Credentials disabled in size experiment")))')
        if "notifications" in capabilities:
            cargo = drop_dependency(cargo, "notify-rust")
            path = vendor / "src/linux/system_notifications.rs"
            contents = path.read_text()
            contents = replace_function(contents, "    pub(crate) fn show(", '        let _ = (app_name, notification);\n        log::warn!("System notifications disabled in size experiment");')
            path.write_text(contents)
        if "inhibit" in capabilities:
            cargo = cargo.replace('    "inhibit",\n', '')
            source = source.replace('        inhibit::{InhibitFlags, InhibitOptions, InhibitProxy},\n', '')
            source = replace_function(source, "async fn inhibit_idle_sleep(", '    let _ = (reason, executor);\n    Err(anyhow!("Idle inhibition disabled in size experiment"))')
        platform.write_text(source)
        (vendor / "Cargo.toml").write_text(cargo)
    if codecs:
        for package in ("gpui-pre", "gpui-pre-wgpu"):
            vendor = work / "experiment-vendor" / package
            shutil.copytree(REGISTRY / f"{package}-0.3.11", vendor)
            patches.append((package, vendor))
            path = vendor / "Cargo.toml"
            contents = path.read_text()
            start = contents.index("[dependencies.image]")
            end = contents.index("\n[", start + 1)
            block = contents[start:end]
            for codec in ("dds", "exr", "ff", "hdr", "pnm", "qoi", "tga", "tiff"):
                old = f'    "{codec}",\n'
                if block.count(old) != 1:
                    raise ValueError((package, codec))
                block = block.replace(old, "")
            path.write_text(contents[:start] + block + contents[end:])
    with manifest.open("a") as output:
        if patches:
            output.write('\n[patch.crates-io]\n')
            for package, vendor in patches:
                output.write(f'{package} = {{ path = "experiment-vendor/{package}" }}\n')
        if mixed:
            for package in ("ropy", "ropy-core", "memchr", "redb", "image", "png:0.18.1", "png:0.17.16", "zune-jpeg", "image-webp"):
                output.write(f'\n[profile.release.package."{package}"]\nopt-level = 3\n')
    if mixed:
        source = manifest.read_text()
        source = source.replace('[profile.release]\n', '[profile.release]\nopt-level = "s"\n')
        manifest.write_text(source)
    return work


variants = [
    ("control", (), False, False),
    ("no-credentials", ("credentials",), False, False),
    ("no-notifications", ("notifications",), False, False),
    ("no-inhibit", ("inhibit",), False, False),
    ("common-codecs", (), True, False),
    ("combined", ("credentials", "notifications", "inhibit"), True, False),
    ("mixed", (), False, True),
]
results = {}
for name, capabilities, codecs, mixed in variants:
    print(f"EXPERIMENT_START {name}", flush=True)
    try:
        work = prepare(name, capabilities, codecs, mixed)
        env = dict(BASE_ENV, CARGO_TARGET_DIR=str(ROOT / ("mixed-build" if mixed else "shared-build")))
        # Local patch overrides necessarily change source IDs in the copied lockfile.
        elapsed = run(["cargo", "build", "--offline", "--release", "-p", "ropy"], work, ROOT / f"build-{name}.log", env)
        binary = ROOT / f"ropy-{name}"
        shutil.copy2(Path(env["CARGO_TARGET_DIR"]) / "release/ropy", binary)
        staging = ROOT / f"archive-{name}"
        staging.mkdir()
        shutil.copy2(binary, staging / "ropy")
        tar = ROOT / f"{name}.tar"
        run(["tar", "--sort=name", "--mtime=@0", "--owner=0", "--group=0", "--numeric-owner", "-cf", str(tar), "-C", str(staging), "ropy"], work, ROOT / f"tar-{name}.log", env)
        with (ROOT / f"{name}.tar.xz").open("wb") as output:
            subprocess.run(["xz", "-T1", "-9", "-c", str(tar)], stdout=output, check=True)
        run(["cargo", "tree", "--locked", "-e", "normal,features"], work, ROOT / f"features-{name}.txt", env)
        results[name] = dict(binary_bytes=binary.stat().st_size, archive_bytes=(ROOT / f"{name}.tar.xz").stat().st_size, build_seconds=elapsed, sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
        if name in ("control", "mixed"):
            run(["cargo", "bench", "--locked", "-p", "ropy-core", "--bench", "search", "--bench", "repository", "--no-run"], work, ROOT / f"bench-build-{name}.log", env)
            # Keep each version's benchmark executables before the shared cache changes.
            paths = list((Path(env["CARGO_TARGET_DIR"]) / "release/deps").glob("search-*")) + list((Path(env["CARGO_TARGET_DIR"]) / "release/deps").glob("repository-*"))
            dest = ROOT / f"benches-{name}"
            dest.mkdir()
            for path in paths:
                if path.is_file() and os.access(path, os.X_OK):
                    shutil.copy2(path, dest / path.name.split('-')[0])
    except Exception as error:
        results[name] = dict(error=str(error))
    (ROOT / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(f"EXPERIMENT_DONE {name} {results[name]}", flush=True)
# Execute binaries directly, without concurrent compilation or accidental recompilation.
for round_number, order in [(1, ("control", "mixed")), (2, ("mixed", "control"))]:
    for name in order:
        if "error" in results.get(name, {}):
            continue
        env = dict(BASE_ENV, CRITERION_HOME=str(ROOT / f"criterion-{name}"))
        for bench in ("search", "repository"):
            run([str(ROOT / f"benches-{name}" / bench), "--bench", "--save-baseline", f"round-{round_number}"], ROOT, ROOT / f"{bench}-{name}-round-{round_number}.log", env)
print("EXPERIMENTS_COMPLETE", flush=True)
