"""Relocation-independent method-body fingerprints.

A body is decoded instruction by instruction. Immediates that a rebuild renumbers or relocates are
replaced by build-independent identities; every other fact stays literal:

* call/ref.func targets become the callee's metadata method name, its import name, or the
  shallow skeleton hash of a runtime helper or a folded body;
* absolute addresses (i32 constants and load/store offsets of at least 64 KiB) become the
  resolved metadata usage (type, method, field, literal SHA-256, RVA field contents SHA-256),
  `bss` for the zero-initialised static region, or a description of the initialised bytes they
  point at (C strings by text, pointers masked);
* usage targets named as per-build pins (gateway build, opcode seed, ...) become `volatile`;
* call_indirect type indices become their signature;
* the order of the IL2CPP method-initialisation calls is canonicalised, because the compiler
  orders them by content-hash names.

Opcode order, locals, branch depths, alignment, object field offsets and every other constant
(opcodes, sizes, waits, timeouts) remain part of the fingerprint.
"""

import bisect
import difflib
import hashlib
import struct

from .wasm import check, decode

ADDRESS_FLOOR = 0x10000
WINDOW = 4


class Fingerprinter:
    def __init__(self, runtime, volatile=None):
        self.rt = runtime
        self.module = runtime.module
        self.cells = usage_region(runtime)
        self.data_end = self.module.data_end()
        self.bss = (self.data_end, stack_pointer(self.module))
        check(self.bss[0] < self.bss[1], "static region bounds")
        segments = sorted((a, n) for a, n, _ in self.module.segments)
        self._starts = [a for a, _ in segments]
        self._segments = segments
        self.volatile = dict(volatile or {})
        self._skeletons = {}

    # Absolute addresses.

    def initialised(self, address):
        i = bisect.bisect_right(self._starts, address) - 1
        return i >= 0 and address < self._segments[i][0] + self._segments[i][1]

    def address(self, value):
        """Build-independent text for an absolute address, or None for an ordinary constant."""
        lo, hi = self.cells
        if lo <= value < hi:
            usage = self.rt.usage(value)
            if usage is not None:
                text = self.rt.describe_usage(*usage)
                name = self.volatile.get(text)
                return "volatile " + name if name else "usage " + text
        if self.bss[0] <= value < self.bss[1]:
            return "bss"
        if ADDRESS_FLOOR <= value < self.data_end and self.initialised(value):
            return "data " + self.data_text(value)
        return None

    def cstring(self, address, limit=256):
        mem = self.rt.mem
        end = mem.find(b"\0", address, address + limit + 1)
        if end - address < 2:
            return None
        text = bytes(mem[address:end])
        if not all(0x20 <= b < 0x7F for b in text):
            return None
        return text.decode("ascii")

    def data_text(self, address):
        text = self.cstring(address)
        if text is not None:
            return "cstr %r" % text
        parts = []
        for word in struct.unpack_from("<%dI" % WINDOW, self.rt.mem, address):
            if ADDRESS_FLOOR <= word < self.data_end:
                target = self.cstring(word)
                parts.append("p" if target is None else "s:%r" % target)
            else:
                parts.append("%x" % word)
        return " ".join(parts)

    # Calls.

    def callee(self, function):
        imported = self.module.imported_functions
        if function < len(imported):
            module, name, _, _ = imported[function]
            return "import %s.%s" % (module, name)
        methods = self.rt.function_methods.get(function, ())
        names = sorted({self.rt.method_name(i) for i in methods})
        if len(names) == 1:
            return "method " + names[0]
        prefix = "folded" if names else "helper"
        return "%s %s" % (prefix, self.skeleton(function))

    def skeleton(self, function):
        """Shallow identity of a runtime helper or folded body: opcodes with all relocatable
        immediates masked. It never recurses, so it cannot depend on unrelated renumbering."""
        cached = self._skeletons.get(function)
        if cached is not None:
            return cached
        locals_, instructions = decode(self.module, function)
        h = hashlib.sha256(self.module.function_signature(function).encode())
        h.update(repr(locals_).encode())
        imported = self.module.imported_functions
        for ins in instructions:
            if ins.op in ("call", "return_call", "ref.func"):
                target = ins.imm[0]
                text = ins.op + (" " + imported[target][1] if target < len(imported) else "")
            elif ins.op in ("call_indirect", "return_call_indirect"):
                text = ins.op + " " + self.module.signature(ins.imm[0])
            elif ins.op == "i32.const":
                text = "i32.const %s" % (ins.imm[0] if -1024 < ins.imm[0] < 1024 else "C")
            elif is_memory(ins) and ins.imm[1] >= ADDRESS_FLOOR:
                text = "%s %d C" % (ins.op, ins.imm[0])
            else:
                text = ins.text()
            h.update(text.encode() + b"\n")
        digest = h.hexdigest()[:24]
        self._skeletons[function] = digest
        return digest

    # Bodies.

    def tokens(self, function):
        """(locals, [(original offset, normalized text)]) in original instruction order."""
        locals_, instructions = decode(self.module, function)
        out = []
        for ins in instructions:
            if ins.op in ("call", "return_call", "ref.func"):
                text = ins.op + " " + self.callee(ins.imm[0])
            elif ins.op in ("call_indirect", "return_call_indirect"):
                text = "%s %s table %d" % (ins.op, self.module.signature(ins.imm[0]), ins.imm[1])
            elif ins.op == "i32.const":
                text = "i32.const " + (self.address(ins.imm[0]) or str(ins.imm[0]))
            elif ins.imm and ins.imm[0] == "ty":
                text = "%s type %s" % (ins.op, self.module.signature(ins.imm[1]))
            elif is_memory(ins) and ins.imm[1] >= ADDRESS_FLOOR:
                where = self.address(ins.imm[1])
                text = "%s %d %s" % (ins.op, ins.imm[0], where or ins.imm[1])
            else:
                text = ins.text()
            out.append((ins.offset, text))
        return locals_, out

    def normalized(self, function):
        """(locals, [(offset, text)]) with method-initialisation runs canonicalised."""
        locals_, tokens = self.tokens(function)
        return locals_, canonical_initialisation(tokens, self.initialiser_calls(function))

    def initialiser_calls(self, function):
        """Texts of calls that take one i32 and return nothing: candidates for the IL2CPP
        metadata initialiser whose call order carries no meaning."""
        out = set()
        _, instructions = decode(self.module, function)
        for ins in instructions:
            if ins.op == "call" and self.module.function_signature(ins.imm[0]) == "(i32)->()":
                if not self.rt.function_methods.get(ins.imm[0]):
                    out.add("call " + self.callee(ins.imm[0]))
        return out

    def fingerprint(self, function):
        """SHA-256 of the normalized body and its token count."""
        locals_, tokens = self.normalized(function)
        h = hashlib.sha256(self.module.function_signature(function).encode())
        h.update(repr(locals_).encode())
        for _, text in tokens:
            h.update(text.encode() + b"\n")
        return h.hexdigest(), len(tokens)

    def usages(self, function):
        """Distinct usage targets of one body in first-use order: (cell, kind, index)."""
        _, instructions = decode(self.module, function)
        lo, hi = self.cells
        seen = []
        for ins in instructions:
            if ins.op == "i32.const" and lo <= ins.imm[0] < hi:
                usage = self.rt.usage(ins.imm[0])
                if usage is not None and (ins.imm[0],) + usage not in seen:
                    seen.append((ins.imm[0],) + usage)
        return seen


def is_memory(ins):
    return len(ins.imm) == 2 and (".load" in ins.op or ".store" in ins.op)


def canonical_initialisation(tokens, calls):
    """Sort maximal runs of `i32.const usage/volatile; call <initialiser>` pairs.

    Each pair initialises one metadata cell and has no data dependency on its neighbours (the
    callee takes one i32 and returns nothing), so the order inside a run is not semantic."""
    out = list(tokens)
    i = 0
    while i < len(out) - 1:
        j = i
        while (j + 1 < len(out) and out[j + 1][1] in calls
               and out[j][1].startswith(("i32.const usage ", "i32.const volatile "))
               and out[j + 1][1] == out[i + 1][1]):
            j += 2
        if j - i >= 4:
            pairs = sorted((out[k][1], out[k + 1][1]) for k in range(i, j, 2))
            for n, (a, b) in enumerate(pairs):
                out[i + 2 * n] = (out[i + 2 * n][0], a)
                out[i + 2 * n + 1] = (out[i + 2 * n + 1][0], b)
        i = max(j, i + 1)
    return out


def usage_region(rt):
    """The contiguous memory range holding unresolved metadata-usage cells.

    Cells are U32 words `(kind << 29) | (index << 1) | 1`. The range is the span of runs of at
    least 256 consecutive valid cells; at least 95% of its words must be cells or zero.
    """
    mem, end = rt.mem, rt.module.data_end()
    words = struct.unpack_from("<%dI" % (end // 4), mem, 0)
    runs = []
    start = None
    for i, raw in enumerate(words):
        ok = raw & 1 and rt.usage(i * 4) is not None
        if ok and start is None:
            start = i
        elif not ok and start is not None:
            if i - start >= 256:
                runs.append((start, i))
            start = None
    check(runs, "no metadata-usage cell region")
    lo, hi = runs[0][0], runs[-1][1]
    cells = sum(1 for i in range(lo, hi) if words[i] == 0 or (words[i] & 1 and rt.usage(i * 4)))
    check(cells >= 0.95 * (hi - lo), "metadata-usage region is not dense")
    return lo * 4, hi * 4


def stack_pointer(module):
    """Initial value of global 0 (the C stack pointer), which bounds the static region."""
    c = module._section(6)
    check(c.uleb() > 0, "no globals")
    c.byte()
    c.byte()
    check(c.byte() == 0x41, "global 0 is not an i32 constant")
    return c.sleb(32)


def diff(a, b, context=2):
    """Unified diff of two normalized token lists, for review output."""
    return list(difflib.unified_diff([t for _, t in a], [t for _, t in b], "baseline", "target",
                                     lineterm="", n=context))
