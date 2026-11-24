"""Headless Ghidra pipeline: build/JA.elf + project knowledge -> Ghidra project + pseudocode for every function.

usage: ./t ghidra [--no-decomp]
Steps: ./t le2elf (labels.csv -> ELF symbols), ./t jatypes --json, then analyzeHeadless imports build/JA.elf into
build/ghidra/JA (replacing it), auto-analyses, runs tools/ghidra/ApplyJA.java (__watcall convention, defs/cdecl.txt,
structs + typed globals from defs/types.toml, labels.csv comments) and ExportDecomp.java -> analysis/decomp/
(generated, gitignored), then refreshes analysis/strrefs.txt (procedure ranges come from the new decomp; strings.txt
is created first if missing). Open build/ghidra/JA.gpr in the Ghidra GUI to browse the same state.
Takes a few minutes; log in build/ghidra/headless.log.
"""
import os
import shutil
import subprocess
import sys
from common import ROOT

GHIDRA = os.environ.get("GHIDRA_HOME", "/opt/ghidra")
JAVA_HOME = "/usr/lib/jvm/java-21-openjdk"


def main():
    t = os.path.join(ROOT, "t")
    subprocess.run([t, "le2elf"], check=True)
    subprocess.run([t, "jatypes", "--json"], check=True)
    proj = os.path.join(ROOT, "build", "ghidra")
    shutil.rmtree(proj, ignore_errors=True)
    os.makedirs(proj)
    cmd = [os.path.join(GHIDRA, "support", "analyzeHeadless"), proj, "JA",
           "-import", os.path.join(ROOT, "build", "JA.elf"),
           "-scriptPath", os.path.join(ROOT, "tools", "ghidra"),
           "-postScript", "ApplyJA.java", ROOT]
    if "--no-decomp" not in sys.argv:
        cmd += ["-postScript", "ExportDecomp.java", ROOT]
    env = dict(os.environ, JAVA_HOME=JAVA_HOME, PATH=f"{JAVA_HOME}/bin:" + os.environ["PATH"])
    log = os.path.join(proj, "headless.log")
    with open(log, "w") as f:
        r = subprocess.run(cmd, env=env, stdout=f, stderr=subprocess.STDOUT)
    for line in open(log):
        if "ApplyJA.java>" in line or "ExportDecomp.java>" in line or "ERROR" in line:
            print(line.rstrip())
    if r.returncode == 0 and "--no-decomp" not in sys.argv:
        if not os.path.exists(os.path.join(ROOT, "analysis", "strings.txt")):
            subprocess.run([t, "strings_dump", "--write"], check=True, stdout=subprocess.DEVNULL)
        subprocess.run([t, "strrefs", "--write"], check=True, stdout=subprocess.DEVNULL)
        print("analysis/strrefs.txt refreshed")
    sys.exit(r.returncode)


if __name__ == "__main__":
    main()
