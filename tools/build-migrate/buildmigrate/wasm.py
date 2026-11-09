"""Static WebAssembly reader. It parses bytes only; nothing is instantiated."""

import struct


class Malformed(ValueError):
    pass


def check(ok, why):
    if not ok:
        raise Malformed(why)


class Cursor:
    def __init__(self, data, pos=0, end=None):
        self.data = data
        self.pos = pos
        self.end = len(data) if end is None else end

    def byte(self):
        check(self.pos < self.end, "truncated byte")
        value = self.data[self.pos]
        self.pos += 1
        return value

    def take(self, n):
        check(0 <= n <= self.end - self.pos, "truncated bytes")
        value = self.data[self.pos:self.pos + n]
        self.pos += n
        return value

    def uleb(self, bits=32):
        value = shift = 0
        while True:
            b = self.byte()
            value |= (b & 0x7F) << shift
            shift += 7
            if b < 0x80:
                check(value < (1 << bits), "uleb overflow")
                return value
            check(shift < bits + 7, "uleb too long")

    def sleb(self, bits):
        value = shift = 0
        while True:
            b = self.byte()
            value |= (b & 0x7F) << shift
            shift += 7
            if b < 0x80:
                if b & 0x40:
                    value -= 1 << shift
                check(-(1 << (bits - 1)) <= value < (1 << (bits - 1)), "sleb overflow")
                return value
            check(shift < bits + 7, "sleb too long")

    def name(self):
        return self.take(self.uleb()).decode("utf-8")


VALTYPES = {0x7F: "i32", 0x7E: "i64", 0x7D: "f32", 0x7C: "f64", 0x7B: "v128",
            0x70: "funcref", 0x6F: "externref"}


def const_expr(c):
    """Return the value of a single i32.const ... end constant expression."""
    op = c.byte()
    check(op == 0x41, "unsupported constant expression")
    value = c.sleb(32)
    check(c.byte() == 0x0B, "unterminated constant expression")
    return value


class Module:
    """Sections, imports, element table, reconstructed data memory and code bodies."""

    def __init__(self, data):
        check(data[:8] == b"\0asm\x01\0\0\0", "not a version-1 WASM module")
        self.data = data
        self.sections = {}
        c = Cursor(data, 8)
        while c.pos < len(data):
            kind = c.byte()
            size = c.uleb()
            start = c.pos
            check(start + size <= len(data), "section extent")
            if kind != 0:
                check(kind not in self.sections, "duplicate section")
                self.sections[kind] = (start, start + size)
            c.pos = start + size
        self.types = self._types()
        self.imports = self._imports()
        self.imported_functions = [i for i in self.imports if i[2] == 0]
        self.function_types = self._functions()
        self.elements = self._elements()
        self.segments = self._data()
        self.bodies = self._code()
        check(len(self.bodies) == len(self.function_types), "function/code count mismatch")

    def _section(self, kind):
        start, end = self.sections[kind]
        return Cursor(self.data, start, end)

    def _types(self):
        c = self._section(1)
        out = []
        for _ in range(c.uleb()):
            check(c.byte() == 0x60, "unsupported type form")
            params = tuple(VALTYPES.get(c.byte(), "?") for _ in range(c.uleb()))
            results = tuple(VALTYPES.get(c.byte(), "?") for _ in range(c.uleb()))
            out.append((params, results))
        return out

    def _imports(self):
        c = self._section(2)
        out = []
        for _ in range(c.uleb()):
            module, field = c.name(), c.name()
            kind = c.byte()
            if kind == 0:
                desc = c.uleb()
            elif kind == 1:
                c.byte()
                desc = self._limits(c)
            elif kind == 2:
                desc = self._limits(c)
            elif kind == 3:
                desc = (c.byte(), c.byte())
            elif kind == 4:
                desc = (c.byte(), c.uleb())
            else:
                raise Malformed("unknown import kind")
            out.append((module, field, kind, desc))
        return out

    @staticmethod
    def _limits(c):
        flags = c.byte()
        low = c.uleb()
        high = c.uleb() if flags & 1 else None
        return (flags, low, high)

    def _functions(self):
        c = self._section(3)
        return [c.uleb() for _ in range(c.uleb())]

    def _elements(self):
        c = self._section(9)
        table = []
        for _ in range(c.uleb()):
            check(c.uleb() == 0, "unsupported element segment form")
            offset = const_expr(c)
            values = [c.uleb() for _ in range(c.uleb())]
            if len(table) < offset:
                table.extend([None] * (offset - len(table)))
            table[offset:offset + len(values)] = values
        return table

    def _data(self):
        c = self._section(11)
        out = []
        for _ in range(c.uleb()):
            flags = c.uleb()
            if flags == 1:
                c.take(c.uleb())
                continue
            if flags == 2:
                check(c.uleb() == 0, "unsupported memory index")
            else:
                check(flags == 0, "unsupported data segment form")
            address = const_expr(c)
            size = c.uleb()
            out.append((address, size, c.pos))
            c.take(size)
        return out

    def _code(self):
        c = self._section(10)
        out = []
        for _ in range(c.uleb()):
            size = c.uleb()
            out.append((c.pos, size))
            c.take(size)
        return out

    def memory(self, size=64 * 1024 * 1024):
        mem = bytearray(size)
        for address, n, at in self.segments:
            check(0 <= address and address + n <= size, "data segment outside memory")
            mem[address:address + n] = self.data[at:at + n]
        return mem

    def data_end(self):
        return max(address + n for address, n, _ in self.segments)

    def body(self, function):
        """Original (start, size) of a defined function body, local declarations included."""
        index = function - len(self.imported_functions)
        check(0 <= index < len(self.bodies), "function is imported or out of range")
        return self.bodies[index]

    def signature(self, type_index):
        params, results = self.types[type_index]
        return "(" + ",".join(params) + ")->(" + ",".join(results) + ")"

    def function_signature(self, function):
        n = len(self.imported_functions)
        if function < n:
            return self.signature(self.imported_functions[function][3])
        return self.signature(self.function_types[function - n])


# Immediate kinds: '' none, bt blocktype, l label, brt br_table, f function, ci call_indirect,
# loc local, glb global, tbl table, mem memarg, b0 reserved byte, i32/i64/f32/f64 constants,
# sel typed select, rt reference type, tag exception tag, ty type index.
_OPS = {
    0x00: ("unreachable", ""), 0x01: ("nop", ""), 0x02: ("block", "bt"), 0x03: ("loop", "bt"),
    0x04: ("if", "bt"), 0x05: ("else", ""), 0x06: ("try", "bt"), 0x07: ("catch", "tag"),
    0x08: ("throw", "tag"), 0x09: ("rethrow", "l"), 0x0A: ("throw_ref", ""), 0x0B: ("end", ""),
    0x0C: ("br", "l"), 0x0D: ("br_if", "l"), 0x0E: ("br_table", "brt"), 0x0F: ("return", ""),
    0x10: ("call", "f"), 0x11: ("call_indirect", "ci"), 0x12: ("return_call", "f"),
    0x13: ("return_call_indirect", "ci"), 0x14: ("call_ref", "ty"), 0x15: ("return_call_ref", "ty"),
    0x18: ("delegate", "l"), 0x19: ("catch_all", ""), 0x1A: ("drop", ""), 0x1B: ("select", ""),
    0x1C: ("select_t", "sel"), 0x20: ("local.get", "loc"), 0x21: ("local.set", "loc"),
    0x22: ("local.tee", "loc"), 0x23: ("global.get", "glb"), 0x24: ("global.set", "glb"),
    0x25: ("table.get", "tbl"), 0x26: ("table.set", "tbl"), 0x3F: ("memory.size", "b0"),
    0x40: ("memory.grow", "b0"), 0x41: ("i32.const", "i32"), 0x42: ("i64.const", "i64"),
    0x43: ("f32.const", "f32"), 0x44: ("f64.const", "f64"), 0xD0: ("ref.null", "rt"),
    0xD1: ("ref.is_null", ""), 0xD2: ("ref.func", "f"), 0xD3: ("ref.as_non_null", ""),
    0xD4: ("br_on_null", "l"), 0xD5: ("ref.eq", ""), 0xD6: ("br_on_non_null", "l"),
}
_MEMORY = ("i32.load i64.load f32.load f64.load i32.load8_s i32.load8_u i32.load16_s "
           "i32.load16_u i64.load8_s i64.load8_u i64.load16_s i64.load16_u i64.load32_s "
           "i64.load32_u i32.store i64.store f32.store f64.store i32.store8 i32.store16 "
           "i64.store8 i64.store16 i64.store32").split()
for _n, _name in enumerate(_MEMORY):
    _OPS[0x28 + _n] = (_name, "mem")
for _op in range(0x45, 0xC5):
    _OPS[_op] = ("num.%02x" % _op, "")
_NAMED = {0x45: "i32.eqz", 0x46: "i32.eq", 0x47: "i32.ne", 0x48: "i32.lt_s", 0x49: "i32.lt_u",
          0x4A: "i32.gt_s", 0x4B: "i32.gt_u", 0x4C: "i32.le_s", 0x4D: "i32.le_u",
          0x4E: "i32.ge_s", 0x4F: "i32.ge_u", 0x6A: "i32.add", 0x6B: "i32.sub", 0x6C: "i32.mul",
          0x70: "i32.rem_u", 0x71: "i32.and", 0x72: "i32.or", 0x73: "i32.xor", 0x74: "i32.shl",
          0x75: "i32.shr_s", 0x76: "i32.shr_u", 0x77: "i32.rotl", 0x78: "i32.rotr"}
for _op, _name in _NAMED.items():
    _OPS[_op] = (_name, "")
_FC = {8: "data_mem", 9: "data", 10: "b0b0", 11: "b0", 12: "elem_tbl", 13: "elem",
       14: "tbl_tbl", 15: "tbl", 16: "tbl", 17: "tbl"}


class Instruction:
    __slots__ = ("offset", "op", "imm")

    def __init__(self, offset, op, imm):
        self.offset = offset
        self.op = op
        self.imm = imm

    def text(self):
        return self.op + ("" if self.imm == () else " " + " ".join(map(str, self.imm)))


def _blocktype(c):
    b = c.data[c.pos]
    if b == 0x40 or b in VALTYPES:
        c.pos += 1
        return ("bt", b)
    return ("ty", c.sleb(33))


def _memarg(c):
    align = c.uleb()
    check(align < 64, "multi-memory memarg is unsupported")
    return (align, c.uleb())


def decode(module, function):
    """Decode one original body into locals and instructions with original file offsets."""
    start, size = module.body(function)
    c = Cursor(module.data, start, start + size)
    local_groups = tuple((c.uleb(), c.byte()) for _ in range(c.uleb()))
    out = []
    depth = 1
    while depth:
        at = c.pos
        op = c.byte()
        if op in (0xFC, 0xFD, 0xFE):
            sub = c.uleb()
            out.append(Instruction(at, "%02x.%d" % (op, sub), _prefixed(c, op, sub)))
            continue
        check(op in _OPS, "unknown opcode 0x%02x at 0x%x" % (op, at))
        name, kind = _OPS[op]
        if kind == "":
            imm = ()
        elif kind == "bt":
            imm = _blocktype(c)
        elif kind in ("l", "f", "loc", "glb", "tbl", "tag", "ty"):
            imm = (c.uleb(),)
        elif kind == "brt":
            imm = tuple(c.uleb() for _ in range(c.uleb() + 1))
        elif kind == "ci":
            imm = (c.uleb(), c.uleb())
        elif kind == "mem":
            imm = _memarg(c)
        elif kind == "b0":
            check(c.byte() == 0, "nonzero reserved byte")
            imm = ()
        elif kind == "i32":
            imm = (c.sleb(32),)
        elif kind == "i64":
            imm = (c.sleb(64),)
        elif kind == "f32":
            imm = (c.take(4).hex(),)
        elif kind == "f64":
            imm = (c.take(8).hex(),)
        elif kind == "sel":
            imm = tuple(c.byte() for _ in range(c.uleb()))
        elif kind == "rt":
            imm = (c.byte(),)
        else:
            raise Malformed("unhandled immediate kind")
        if op in (0x02, 0x03, 0x04, 0x06):
            depth += 1
        elif op == 0x0B:
            depth -= 1
        elif op == 0x18:
            depth -= 1
        out.append(Instruction(at, name, imm))
    check(c.pos == start + size, "body did not end at its declared size")
    return local_groups, out


def _prefixed(c, op, sub):
    if op == 0xFC:
        kind = _FC.get(sub, "")
        check(sub <= 17, "unknown 0xfc opcode")
        if kind == "":
            return ()
        if kind == "data_mem":
            index = c.uleb()
            check(c.byte() == 0, "memory index")
            return (index,)
        if kind in ("data", "elem", "tbl"):
            return (c.uleb(),)
        if kind == "b0b0":
            check(c.byte() == 0 and c.byte() == 0, "memory index")
            return ()
        if kind == "b0":
            check(c.byte() == 0, "memory index")
            return ()
        return (c.uleb(), c.uleb())
    if op == 0xFD:
        if sub <= 11 or sub in (92, 93):
            return _memarg(c)
        if sub in (12, 13):
            return (c.take(16).hex(),)
        if 21 <= sub <= 34:
            return (c.byte(),)
        if 84 <= sub <= 91:
            return _memarg(c) + (c.byte(),)
        return ()
    if sub == 0x03:
        check(c.byte() == 0, "atomic.fence flags")
        return ()
    return _memarg(c)


def u32(mem, address):
    return struct.unpack_from("<I", mem, address)[0]
