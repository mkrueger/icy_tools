import sys
import os

if len(sys.argv) != 3:
    print("need 2 arguments")
    sys.exit(1)

version=""
cargo = open(os.path.join("crates", sys.argv[1], "Cargo.toml"), "r")
for line in cargo.readlines():
    if line.startswith("version"):
        m = line.index('"')
        version = line[m + 1:len(line) - 2]
        break
cargo.close()

file_id = open(os.path.join("crates", sys.argv[1], "build", "file_id.diz"), "r")
lines = file_id.readlines()
file_id.close()
new_lines = list(map(lambda line: line.replace("#VERSION", version), lines))

f = open(sys.argv[2], "w")
f.writelines(new_lines)
f.close()

print(version)
