// Applies the project's knowledge to build/JA.elf in a headless Ghidra project (run by tools/ghidra.py):
//  - adds the Watcom register calling convention "__watcall" as a program spec extension (args EAX EDX EBX ECX then
//    stack, result EAX, callee saves everything else) and makes it the convention of every function except the
//    cdecl ones listed in defs/cdecl.txt (AIL, some clib)
//  - creates the structs of defs/types.toml (via build/ja_types.json) and types the globals
//  - function signatures of defs/functions.toml (parameter + return types; storage follows the convention)
//  - plate comments from labels.csv
// @category JA
import com.google.gson.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.database.SpecExtension;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.listing.Function.FunctionUpdateType;
import ghidra.program.model.symbol.SourceType;
import java.io.*;
import java.nio.file.*;
import java.util.*;

public class ApplyJA extends GhidraScript {
    static final String WATCALL =
        "<prototype name=\"__watcall\" extrapop=\"unknown\" stackshift=\"4\">\n" +
        "  <input>\n" +
        "    <pentry minsize=\"1\" maxsize=\"4\"><register name=\"EAX\"/></pentry>\n" +
        "    <pentry minsize=\"1\" maxsize=\"4\"><register name=\"EDX\"/></pentry>\n" +
        "    <pentry minsize=\"1\" maxsize=\"4\"><register name=\"EBX\"/></pentry>\n" +
        "    <pentry minsize=\"1\" maxsize=\"4\"><register name=\"ECX\"/></pentry>\n" +
        "    <pentry minsize=\"1\" maxsize=\"500\" align=\"4\"><addr offset=\"4\" space=\"stack\"/></pentry>\n" +
        "  </input>\n" +
        "  <output>\n" +
        "    <pentry minsize=\"1\" maxsize=\"4\"><register name=\"EAX\"/></pentry>\n" +
        "  </output>\n" +
        "  <unaffected>\n" +
        "    <register name=\"EBX\"/><register name=\"ECX\"/><register name=\"EDX\"/><register name=\"ESI\"/>\n" +
        "    <register name=\"EDI\"/><register name=\"EBP\"/><register name=\"ESP\"/>\n" +
        "  </unaffected>\n" +
        "</prototype>\n";

    DataTypeManager dtm;
    Map<String, DataType> structs = new HashMap<>();
    CategoryPath cat = new CategoryPath("/JA");

    DataType prim(String b) {
        switch (b) {
            case "u8": return ByteDataType.dataType;
            case "i8": return SignedByteDataType.dataType;
            case "u16": return WordDataType.dataType;
            case "i16": return ShortDataType.dataType;
            case "u32": return DWordDataType.dataType;
            case "i32": return IntegerDataType.dataType;
            case "ptr": return PointerDataType.dataType;
            case "char": return CharDataType.dataType;
        }
        return structs.get(b);
    }

    DataType parse(String t) {
        java.util.regex.Matcher m = java.util.regex.Pattern.compile("(\\w+)\\s*(\\*?)((?:\\[\\d+\\])*)").matcher(t.trim());
        if (!m.matches()) throw new IllegalArgumentException(t);
        DataType dt = m.group(1).equals("void") ? VoidDataType.dataType : prim(m.group(1));
        if (dt == null) throw new IllegalArgumentException("unknown type " + t);
        if (!m.group(2).isEmpty()) dt = new PointerDataType(dt, 4, dtm);   // T* (32-bit flat)
        java.util.regex.Matcher d = java.util.regex.Pattern.compile("\\[(\\d+)\\]").matcher(m.group(3));
        List<Integer> dims = new ArrayList<>();
        while (d.find()) dims.add(Integer.parseInt(d.group(1)));
        for (int n : dims) dt = new ArrayDataType(dt, n, dt.getLength(), dtm);   // innermost first, as in jatypes
        return dt;
    }

    @Override
    public void run() throws Exception {
        String root = getScriptArgs().length > 0 ? getScriptArgs()[0] : System.getProperty("user.dir");
        JsonObject js = JsonParser.parseReader(new FileReader(root + "/build/ja_types.json")).getAsJsonObject();
        dtm = currentProgram.getDataTypeManager();

        // 1. calling convention
        new SpecExtension(currentProgram).addReplaceCompilerSpecExtension(WATCALL, monitor);
        Set<Long> cdecl = new HashSet<>();
        Path cd = Paths.get(root, "defs", "cdecl.txt");
        if (Files.exists(cd)) {
            for (String line : Files.readAllLines(cd)) {
                line = line.replaceAll("#.*", "").trim();
                if (line.isEmpty()) continue;
                String[] p = line.split("\\s+")[0].split("-");
                long lo = Long.decode(p[0]), hi = p.length > 1 ? Long.decode(p[1]) : lo;
                for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                    long a = f.getEntryPoint().getOffset();
                    if (a >= lo && a <= hi) cdecl.add(a);
                }
            }
        }
        int nw = 0, nc = 0;
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            boolean c = cdecl.contains(f.getEntryPoint().getOffset());
            f.setCallingConvention(c ? "__cdecl" : "__watcall");
            if (c) nc++; else nw++;
        }
        println("calling conventions: __watcall " + nw + ", __cdecl " + nc);

        // 2. structs (two passes: create empty, then fill, so fields can use any struct)
        JsonObject ss = js.getAsJsonObject("structs");
        for (String name : ss.keySet()) {
            StructureDataType s = new StructureDataType(cat, name, ss.getAsJsonObject(name).get("size").getAsInt(), dtm);
            structs.put(name, dtm.addDataType(s, DataTypeConflictHandler.REPLACE_HANDLER));
        }
        for (String name : ss.keySet()) {
            JsonObject so = ss.getAsJsonObject(name);
            Structure s = (Structure) structs.get(name);
            s.setDescription(so.get("doc").getAsString());
            for (JsonElement fe : so.getAsJsonArray("fields")) {
                JsonArray f = fe.getAsJsonArray();
                DataType ft = parse(f.get(1).getAsString());
                String cmt = f.get(3).getAsString() + (f.get(4).getAsString().isEmpty() ? "" : ": " + f.get(4).getAsString());
                s.replaceAtOffset(f.get(0).getAsInt(), ft, ft.getLength(), f.get(2).getAsString(), cmt);
            }
        }
        println("structs: " + structs.size());

        // 3. typed globals
        Listing L = currentProgram.getListing();
        int ng = 0;
        for (JsonElement ge : js.getAsJsonArray("globals")) {
            JsonArray g = ge.getAsJsonArray();
            Address a = toAddr(g.get(0).getAsLong());
            DataType dt = parse(g.get(2).getAsString());
            try {
                L.clearCodeUnits(a, a.add(dt.getLength() - 1), false);
                L.createData(a, dt);
                ng++;
            } catch (Exception e) {
                println("global " + g.get(1).getAsString() + ": " + e.getMessage());
            }
        }
        println("typed globals: " + ng);

        // 4. signatures (after the structs; the storage of each parameter is derived from the function's convention:
        //    __watcall EAX EDX EBX ECX then stack, __cdecl stack)
        int nf = 0;
        for (JsonElement fe : js.getAsJsonArray("functions")) {
            JsonArray fj = fe.getAsJsonArray();
            Address a = toAddr(fj.get(0).getAsLong());
            Function f = getFunctionAt(a);
            if (f == null) f = createFunction(a, fj.get(1).getAsString());
            if (f == null) { println("signature " + fj.get(1).getAsString() + ": no function"); continue; }
            try {
                String rs = fj.get(2).getAsString();          // "?" = keep the analysed return type
                ReturnParameterImpl ret = new ReturnParameterImpl(
                    rs.equals("?") ? f.getReturnType() : parse(rs), currentProgram);
                List<Variable> ps = new ArrayList<>();
                for (JsonElement pe : fj.get(3).getAsJsonArray()) {
                    JsonArray p = pe.getAsJsonArray();
                    String ts = p.get(0).getAsString();          // "?" = unknown 4 bytes
                    DataType pt = ts.equals("?") ? Undefined4DataType.dataType : parse(ts);
                    ps.add(new ParameterImpl(p.get(1).getAsString(), pt, currentProgram));
                }
                f.updateFunction(f.getCallingConventionName(), ret, ps, FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS,
                                 true, SourceType.USER_DEFINED);
                nf++;
            } catch (Exception e) {
                println("signature " + fj.get(1).getAsString() + ": " + e.getMessage());
            }
        }
        println("signatures: " + nf);

        // 5. labels.csv comments as plate comments (names already come from the ELF .symtab)
        int nl = 0;
        for (JsonElement le : js.getAsJsonArray("labels")) {
            JsonArray l = le.getAsJsonArray();
            if (l.get(2).getAsString().isEmpty()) continue;
            Address a = toAddr(l.get(0).getAsLong());
            currentProgram.getListing().setComment(a, CommentType.PLATE, l.get(2).getAsString());
            nl++;
        }
        println("comments: " + nl);
    }
}
