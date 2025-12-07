import glob

for file in glob.glob("../../external/plugins/*.lua"):

    print("fs::write(dir.join(\"" + file[len("../../external/plugins/"):] +  "\"), include_bytes!(\"" + file + "\"))?;")


