"""IL2CPP metadata/WASM joins for the target's custom metadata format 108.

Everything here reads public bytes. Record layouts are derived from the header and checked
against the pinned research layout (30-byte methods, 66-byte types, 10-byte fields).
"""

import hashlib
import struct

from .wasm import Malformed, check, u32

SECTIONS = (
    "stringLiteral stringLiteralData string events properties methods parameterDefaultValues "
    "fieldDefaultValues fieldAndParameterDefaultValueData fieldMarshaledSizes parameters fields "
    "genericParameters genericParameterConstraints genericContainers nestedTypes interfaces "
    "vtableMethods interfaceOffsets typeDefinitions typeInlineArrays images assemblies fieldRefs "
    "referencedAssemblies attributeData attributeDataRange unresolvedVirtualCallParameterTypes "
    "unresolvedVirtualCallParameterRanges windowsRuntimeTypeNames windowsRuntimeStrings "
    "exportedTypeDefinitions methodRefsOnType genericMethodRefs methodSpecs "
    "genericMethodFunctionsDefinitions genericMethodFunctionsDefinitionsWithAdjustor "
    "invokerIndices rgctxRanges rgctxValues staticConstructorTypeIndices").split()
MAGIC = 0xFAB11BAF
VERSION = 108

# Il2CppTypeEnum values.
PRIMITIVES = {1: "System.Void", 2: "System.Boolean", 3: "System.Char", 4: "System.SByte",
              5: "System.Byte", 6: "System.Int16", 7: "System.UInt16", 8: "System.Int32",
              9: "System.UInt32", 10: "System.Int64", 11: "System.UInt64", 12: "System.Single",
              13: "System.Double", 14: "System.String", 0x16: "System.TypedReference",
              0x18: "System.IntPtr", 0x19: "System.UIntPtr", 0x1C: "System.Object"}

# Metadata usage kinds as encoded by this build: (raw >> 29). Verified by the joins in
# docs/research/current-build-protocol-revalidation.md (4 literal, 6 field data, 2 method).
USAGE_TYPE, USAGE_METHOD, USAGE_FIELD, USAGE_LITERAL, USAGE_METHOD_REF, USAGE_FIELD_RVA = 1, 2, 3, 4, 5, 6


def width(count):
    return "B" if count < 255 else "H" if count < 65535 else "I"


class Metadata:
    def __init__(self, blob):
        check(struct.unpack_from("<II", blob) == (MAGIC, VERSION), "metadata magic/version")
        self.blob = blob
        first = struct.unpack_from("<I", blob, 8)[0]
        check((first - 8) % 12 == 0 and (first - 8) // 12 == len(SECTIONS), "header section count")
        self.h = {n: struct.unpack_from("<III", blob, 8 + 12 * i) for i, n in enumerate(SECTIONS)}
        for name, (offset, size, _) in self.h.items():
            check(offset + size <= len(blob), "section extent " + name)
        w = lambda n: width(self.h[n][2])
        self.type_index = {2: "H", 4: "I"}[self.h["interfaceOffsets"][1] // self.h["interfaceOffsets"][2] - 4]
        tw = self.type_index
        type_format = ("II" + tw * 3 + w("genericContainers") + "I" + w("fields") + w("methods")
                       + w("events") + w("properties") + w("nestedTypes") + w("interfaceOffsets")
                       + "I" + w("interfaceOffsets") + "H" * 8 + "II")
        type_keys = ("name namespace byval declaring parent generic flags fieldStart methodStart "
                     "eventStart propertyStart nestedStart interfacesStart vtableStart "
                     "interfaceOffsetsStart methodCount propertyCount fieldCount eventCount "
                     "nestedCount vtableCount interfacesCount interfaceOffsetsCount bitfield token").split()
        self.types = [dict(zip(type_keys, r)) for r in self.records("typeDefinitions", type_format)]
        method_format = ("I" + w("typeDefinitions") + tw + "I" + w("parameters")
                         + w("genericContainers") + "IHHHH")
        method_keys = ("name declaring returnType returnToken parameterStart generic token flags "
                       "iflags slot parameterCount").split()
        self.methods = [dict(zip(method_keys, r)) for r in self.records("methods", method_format)]
        self.fields = [dict(zip(("name", "type", "token"), r))
                       for r in self.records("fields", "I" + tw + "I")]
        self.parameters = [dict(zip(("name", "token", "type"), r))
                           for r in self.records("parameters", "II" + tw)]
        self.generic_parameters = [
            dict(zip(("owner", "name", "constraintsStart", "constraintsCount", "num", "flags"), r))
            for r in self.records("genericParameters", w("genericContainers") + "I"
                                  + w("genericParameterConstraints") + "HHH")]
        image_format = ("II" + w("typeDefinitions") + "I" + w("typeDefinitions") + "I"
                        + w("methods") + "IIIIII" + w("typeDefinitions") + "I")
        image_keys = ("name assembly typeStart typeCount exportedStart exportedCount entrypoint "
                      "token customStart customCount invokerStart rgctxStart rgctxCount "
                      "staticStart staticCount").split()
        self.images = [dict(zip(image_keys, r)) for r in self.records("images", image_format)]
        for image in self.images:
            image["name"] = self.string(image["name"])
            for t in range(image["typeStart"], image["typeStart"] + image["typeCount"]):
                self.types[t]["image"] = image["name"]
        self.field_refs = self.records("fieldRefs", tw + "I")
        self.field_defaults = {r[0]: r for r in self.records("fieldDefaultValues", "I" + tw + "I")}
        # Usage kind 5 indexes these three tables consecutively: methods on a generic type
        # (method, class instance), generic methods (method, method instance), and generic
        # methods on generic types (method, class instance, method instance).
        self.method_refs = self.records("methodRefsOnType", "IH")
        self.generic_method_refs = self.records("genericMethodRefs", "IH")
        self.method_specs = self.records("methodSpecs", "IHH")
        literal_offset, literal_size, literal_count = self.h["stringLiteral"]
        self.literal_offsets = struct.unpack_from("<%dI" % literal_count, blob, literal_offset)
        check(all(a <= b for a, b in zip(self.literal_offsets, self.literal_offsets[1:])),
              "string literal offsets are not ascending")

    def records(self, name, fmt):
        offset, size, count = self.h[name]
        st = struct.Struct("<" + fmt)
        check(st.size * count == size, "record layout " + name)
        return [st.unpack_from(self.blob, offset + i * st.size) for i in range(count)]

    def string(self, index):
        offset, size, _ = self.h["string"]
        start = offset + index
        check(offset <= start < offset + size, "string index")
        return self.blob[start:self.blob.index(b"\0", start)].decode("utf-8")

    def literal(self, index):
        check(0 <= index < len(self.literal_offsets) - 1, "literal index")
        base = self.h["stringLiteralData"][0]
        start, end = self.literal_offsets[index], self.literal_offsets[index + 1]
        return base + start, self.blob[base + start:base + end]

    def default_data(self, field_index):
        row = self.field_defaults.get(field_index)
        check(row is not None, "field has no default data")
        return self.h["fieldAndParameterDefaultValueData"][0] + row[2]


class Runtime:
    """Joins metadata to the WASM element table, code bodies and reconstructed memory."""

    def __init__(self, metadata, module):
        self.meta = metadata
        self.module = module
        self.mem = module.memory()
        self.registration = self._registration()
        self.type_pointers = u32(self.mem, self.registration + 20)
        self.type_pointer_count = u32(self.mem, self.registration + 16)
        self.field_offsets = u32(self.mem, self.registration + 28)
        self.modules = self._modules()
        self.method_function = {}
        self.function_methods = {}
        for index, method in enumerate(metadata.methods):
            joined = self.join(index)
            if joined is None:
                continue
            function = joined[1]
            self.method_function[index] = joined
            self.function_methods.setdefault(function, []).append(index)
        self._type_names = {}
        self._method_names = {}

    def w(self, address):
        check(0 <= address <= len(self.mem) - 4, "memory address")
        return u32(self.mem, address)

    def _registration(self):
        meta = self.meta
        count = len(meta.types)
        needle = struct.pack("<I", count)
        found = []
        at = self.mem.find(needle)
        while at >= 0:
            base = at - 24
            if (base >= 0 and at % 4 == 0 and self.w(base + 32) == count
                    and self._byval_join(base)):
                found.append(base)
            at = self.mem.find(needle, at + 1)
        check(len(found) == 1, "metadata registration is not unique")
        return found[0]

    def _byval_join(self, base):
        pointers, n = self.w(base + 20), self.w(base + 16)
        if not (0 < pointers < len(self.mem) - 4 * n) or n > 1 << 20:
            return False
        for index, t in enumerate(self.meta.types):
            if t["byval"] >= n:
                return False
            p = self.w(pointers + 4 * t["byval"])
            if not (0 < p < len(self.mem) - 8) or self.w(p) != index:
                return False
        return True

    def _modules(self):
        meta, mem, elements = self.meta, self.mem, self.module.elements
        modules = {}
        maxrid = {}
        for m in meta.methods:
            image = meta.types[m["declaring"]].get("image")
            maxrid[image] = max(maxrid.get(image, 0), m["token"] & 0xFFFFFF)
        for image in meta.images:
            name = image["name"]
            if name not in maxrid:
                continue
            at = mem.find(name.encode() + b"\0")
            if at < 0:
                continue
            check(mem.find(name.encode() + b"\0", at + 1) < 0, "duplicate image name " + name)
            candidates = []
            q = mem.find(struct.pack("<I", at))
            while q >= 0:
                if q % 4 == 0 and q + 12 <= len(mem):
                    _, count, pointers = struct.unpack_from("<III", mem, q)
                    if (count == maxrid[name] and 0 < pointers < len(mem) - 4 * count
                            and all(u32(mem, pointers + 4 * i) < len(elements) for i in range(count))):
                        candidates.append((q, count, pointers))
                q = mem.find(struct.pack("<I", at), q + 1)
            if candidates:
                check(len(candidates) == 1, "code-generation module is not unique: " + name)
                modules[name] = candidates[0]
        return modules

    def join(self, index):
        """Metadata method -> (element slot, function, body start, body size), or None."""
        m = self.meta.methods[index]
        image = self.meta.types[m["declaring"]].get("image")
        if image not in self.modules:
            return None
        _, count, pointers = self.modules[image]
        rid = (m["token"] & 0xFFFFFF) - 1
        check(0 <= rid < count, "token outside its module")
        slot = self.w(pointers + 4 * rid)
        if not slot:
            return None
        function = self.module.elements[slot]
        check(function is not None, "empty element slot")
        start, size = self.module.body(function)
        return slot, function, start, size

    # Canonical, build-independent names.

    def type_definition_name(self, index, depth=0):
        """Namespace-qualified definition name, with '/' for nesting and no image prefix."""
        check(depth < 32, "type nesting")
        t = self.meta.types[index]
        name = self.meta.string(t["name"])
        none = (1 << (8 * struct.calcsize("<" + self.meta.type_index))) - 1
        if t["declaring"] != none:
            outer = self.w(self.w(self.type_pointers + 4 * t["declaring"]))
            check(0 <= outer < len(self.meta.types), "declaring type")
            return self.type_definition_name(outer, depth + 1) + "/" + name
        namespace = self.meta.string(t["namespace"])
        return (namespace + "." if namespace else "") + name

    def qualified_type(self, index):
        return "[" + self.meta.types[index].get("image", "?") + "]" + self.type_definition_name(index)

    def type_name(self, type_index):
        """Canonical name of an Il2CppType referenced by metadata TypeIndex."""
        check(0 <= type_index < self.type_pointer_count, "type index")
        return self.type_name_at(self.w(self.type_pointers + 4 * type_index))

    def type_name_at(self, address, depth=0):
        check(depth < 32, "type nesting")
        cached = self._type_names.get(address)
        if cached is not None:
            return cached
        data, bits = self.w(address), self.w(address + 4)
        kind = (bits >> 16) & 0xFF
        byref = "&" if (bits >> 29) & 1 else ""
        if kind in (0x11, 0x12):
            name = self.qualified_type(data)
        elif kind in PRIMITIVES:
            name = PRIMITIVES[kind]
        elif kind in (0x13, 0x1E):
            p = self.meta.generic_parameters[data]
            name = ("!" if kind == 0x13 else "!!") + self.meta.string(p["name"])
        elif kind == 0x0F:
            name = self.type_name_at(data, depth + 1) + "*"
        elif kind == 0x1D:
            name = self.type_name_at(data, depth + 1) + "[]"
        elif kind == 0x14:
            name = self.type_name_at(self.w(data), depth + 1) + "[%d]" % (self.w(data + 4) & 0xFF)
        elif kind == 0x15:
            generic = self.type_name_at(self.w(data), depth + 1)
            name = generic + self.generic_inst(self.w(data + 4), depth + 1)
        else:
            name = "type%02x" % kind
        name += byref
        self._type_names[address] = name
        return name

    def generic_inst(self, address, depth=0):
        if not address:
            return ""
        count, argv = self.w(address), self.w(address + 4)
        check(count < 64, "generic argument count")
        return "<" + ",".join(self.type_name_at(self.w(argv + 4 * i), depth + 1) for i in range(count)) + ">"

    def method_name(self, index):
        cached = self._method_names.get(index)
        if cached is not None:
            return cached
        m = self.meta.methods[index]
        params = ",".join(self.type_name(self.meta.parameters[m["parameterStart"] + i]["type"])
                          for i in range(m["parameterCount"]))
        name = (self.qualified_type(m["declaring"]) + "::" + self.meta.string(m["name"])
                + "(" + params + ")")
        self._method_names[index] = name
        return name

    def field(self, type_definition, ordinal):
        t = self.meta.types[type_definition]
        check(0 <= ordinal < t["fieldCount"], "field ordinal")
        index = t["fieldStart"] + ordinal
        return index, (self.qualified_type(type_definition) + "::"
                       + self.meta.string(self.meta.fields[index]["name"]))

    def field_ref(self, index):
        check(0 <= index < len(self.meta.field_refs), "field reference")
        type_index, ordinal = self.meta.field_refs[index]
        definition = self.w(self.w(self.type_pointers + 4 * type_index))
        check(0 <= definition < len(self.meta.types), "field reference owner")
        return self.field(definition, ordinal)

    def field_offset(self, type_definition, ordinal):
        row = self.w(self.field_offsets + 4 * type_definition)
        check(row != 0, "type has no field offsets")
        return struct.unpack_from("<i", self.mem, row + 4 * ordinal)[0]

    def fields_of(self, type_definition):
        t = self.meta.types[type_definition]
        return [self.meta.string(self.meta.fields[t["fieldStart"] + i]["name"]) for i in range(t["fieldCount"])]

    # Metadata usage cells.

    def usage(self, address):
        """Decode an unresolved usage cell into (kind, index), or None."""
        if address % 4 or not (0 < address < len(self.mem) - 4):
            return None
        raw = u32(self.mem, address)
        if not raw & 1:
            return None
        kind, index = raw >> 29, (raw & 0x1FFFFFFF) >> 1
        bounds = {USAGE_TYPE: self.type_pointer_count, USAGE_METHOD: len(self.meta.methods),
                  USAGE_FIELD: len(self.meta.field_refs),
                  USAGE_LITERAL: len(self.meta.literal_offsets) - 1,
                  USAGE_METHOD_REF: (len(self.meta.method_refs) + len(self.meta.generic_method_refs)
                                     + len(self.meta.method_specs)),
                  USAGE_FIELD_RVA: len(self.meta.field_refs)}
        if kind not in bounds or index >= bounds[kind]:
            return None
        return kind, index

    def rva_data(self, field_index):
        """Default-data bytes of a static-array initializer field, sized by its value type."""
        offset = self.meta.default_data(field_index)
        type_name = self.type_name(self.meta.fields[field_index]["type"])
        size = rva_size(type_name)
        check(size is not None, "unknown RVA field size: " + type_name)
        return offset, self.meta.blob[offset:offset + size]

    def describe_usage(self, kind, index):
        """Build-independent text for a usage target. Literal contents appear only as SHA-256."""
        if kind == USAGE_TYPE:
            return "type " + self.type_name(index)
        if kind == USAGE_METHOD:
            return "method " + self.method_name(index)
        if kind == USAGE_LITERAL:
            _, data = self.meta.literal(index)
            return "literal sha256:%s len:%d" % (hashlib.sha256(data).hexdigest(), len(data))
        if kind in (USAGE_FIELD, USAGE_FIELD_RVA):
            field_index, name = self.field_ref(index)
            if kind == USAGE_FIELD:
                return "field " + name
            _, data = self.rva_data(field_index)
            return "rva %s sha256:%s len:%d" % (name, hashlib.sha256(data).hexdigest(), len(data))
        if kind == USAGE_METHOD_REF:
            refs = self.meta.method_refs
            if index < len(refs):
                method, inst = refs[index]
                return "methodref " + self.method_name(method) + " on " + self.inst_name(inst)
            index -= len(refs)
            if index < len(self.meta.generic_method_refs):
                method, inst = self.meta.generic_method_refs[index]
                return "genericmethod " + self.method_name(method) + " with " + self.inst_name(inst)
            method, cls, inst = self.meta.method_specs[index - len(self.meta.generic_method_refs)]
            return ("genericmethod " + self.method_name(method) + " on " + self.inst_name(cls)
                    + " with " + self.inst_name(inst))
        raise Malformed("unknown usage kind")

    def inst_name(self, index):
        """Generic instance by metadata-registration index (word 3 array)."""
        count, insts = self.w(self.registration + 8), self.w(self.registration + 12)
        check(0 <= index < count, "generic instance index")
        return self.generic_inst(self.w(insts + 4 * index))


def rva_size(type_name):
    marker = "__StaticArrayInitTypeSize="
    if marker in type_name:
        return int(type_name.rsplit(marker, 1)[1].rstrip(">&"))
    return {"System.Byte": 1, "System.SByte": 1, "System.Int16": 2, "System.UInt16": 2,
            "System.Int32": 4, "System.UInt32": 4, "System.Int64": 8, "System.UInt64": 8,
            "System.Single": 4, "System.Double": 8}.get(type_name)
