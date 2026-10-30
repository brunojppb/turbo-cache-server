"""Stage a release version without resolving or updating any dependency."""
import pathlib
import re
import sys
import tomllib

version = sys.argv[1]
number = r'(?:0|[1-9][0-9]*)'
identifier = r'(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)'
semver = rf'{number}\.{number}\.{number}(?:-{identifier}(?:\.{identifier})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?'
if not re.fullmatch(semver, version) or not re.fullmatch(r'[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}', version):
    sys.exit('Expected a Docker-tag-compatible SemVer (max 128 characters, no v prefix or +metadata)')

updates = []
for filename in ['Cargo.toml', 'Cargo.lock']:
    path = pathlib.Path(filename)
    text = path.read_text()
    # Both files have one root package named decay; restrict the replacement to it.
    pattern = r'(\[\[?package\]\]?\nname = "decay"\n(?:edition = "[^"\n]+"\n|rust-version = "[^"\n]+"\n)*version = ")[^"\n]+(")'
    updated, count = re.subn(pattern, lambda match: match[1] + version + match[2], text)
    if count != 1:
        sys.exit(f'Expected exactly one decay package in {filename}')
    data = tomllib.loads(updated)
    packages = data['package']
    package = packages if isinstance(packages, dict) else next(p for p in packages if p['name'] == 'decay')
    if package['version'] != version:
        sys.exit(f'Failed to stage version in {filename}')
    updates.append((path, updated))
for path, updated in updates:
    path.write_text(updated)
