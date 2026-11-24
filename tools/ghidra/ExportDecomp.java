// Decompiles every function to analysis/decomp/<ADDR>_<name>.c and writes analysis/decomp/procedures.json
// ([{address, name, size}], used by tools/common.load_funcs). Run by tools/ghidra.py after ApplyJA.
// @category JA
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.io.*;
import java.nio.file.*;

public class ExportDecomp extends GhidraScript {
    @Override
    public void run() throws Exception {
        String root = getScriptArgs().length > 0 ? getScriptArgs()[0] : System.getProperty("user.dir");
        Path out = Paths.get(root, "analysis", "decomp");
        Files.createDirectories(out);
        try (DirectoryStream<Path> ds = Files.newDirectoryStream(out, "*.c")) {
            for (Path p : ds) Files.delete(p);
        }
        DecompInterface di = new DecompInterface();
        DecompileOptions opt = new DecompileOptions();
        opt.grabFromProgram(currentProgram);
        di.setOptions(opt);
        di.toggleCCode(true);
        di.openProgram(currentProgram);
        StringBuilder js = new StringBuilder("[\n");
        int n = 0, bad = 0;
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            long a = f.getEntryPoint().getOffset();
            String name = f.getName().replaceAll("[^A-Za-z0-9_?.]", "_");
            DecompileResults r = di.decompileFunction(f, 60, monitor);
            String c;
            if (r != null && r.decompileCompleted()) {
                c = r.getDecompiledFunction().getC();
            } else {
                c = "/* decompile failed: " + (r == null ? "null" : r.getErrorMessage()) + " */\n";
                bad++;
            }
            Files.writeString(out.resolve(String.format("%06X_%s.c", a, name)), c);
            js.append(String.format("%s {\"address\": \"0x%X\", \"name\": \"%s\", \"size\": %d}",
                n == 0 ? "" : ",\n", a, f.getName().replace("\\", "\\\\").replace("\"", "\\\""),
                f.getBody().getNumAddresses()));
            n++;
        }
        js.append("\n]\n");
        Files.writeString(out.resolve("procedures.json"), js.toString());
        di.dispose();
        println("decompiled " + n + " functions, " + bad + " failed");
    }
}
