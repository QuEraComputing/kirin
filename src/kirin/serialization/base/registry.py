from dataclasses import dataclass

from kirin import ir, types
from kirin.ir.attrs.types import TypeAttribute

RUNTIME_ENCODE_LOOKUP = {}
RUNTIME_DECODE_LOOKUP = {}
RUNTIME_NAME2TYPE = {}
DIALECTS_LOOKUP = {}


PREFIX = "_method_@"
PARAM_SEP = "->"


def get_str_from_type(typ: TypeAttribute) -> str:
    repr_name = typ.__repr__() if hasattr(typ, "__repr__") else str(type(typ))
    if repr_name in (
        "int",
        "str",
        "float",
        "bool",
        "NoneType",
        "list",
        "tuple",
        "dict",
    ):
        return repr_name
    elif repr_name == "AnyType()":
        return "Any"
    elif repr_name.startswith("IList["):
        return "IList"
    elif repr_name.startswith("Method["):
        return "Method"
    else:
        return "?"


def mangle(
    symbol_name: str | None,
    param_types: tuple[TypeAttribute, ...],
) -> str:

    mangled_name = f"{PREFIX}{symbol_name}"
    if param_types:
        for typ in param_types:
            mangled_name += f"{PARAM_SEP}{get_str_from_type(typ)}"
    return mangled_name


# def demangle(mangled_name: str) -> dict:
#     if not mangled_name.startswith(PREFIX):
#         raise ValueError(f"Invalid mangled name: {mangled_name}")

#     body = mangled_name[len(PREFIX) :]
#     if body == "":
#         raise ValueError(f"Invalid mangled name body: {body}")

#     parts = body.split(PARAM_SEP)
#     symbol_name = parts[0]
#     param_codes = parts[1:] if len(parts) > 1 else []

#     return {"symbol_name": symbol_name, "param_codes": param_codes}


def register_dialect(dialect: ir.Dialect):
    stmt_map: dict[str, type] = {}
    for stmt_cls in dialect.stmts:
        # register under Python class name
        stmt_map[stmt_cls.__name__] = stmt_cls
        # also register under the statement's declared `name` if provided
        stmt_declared_name = getattr(stmt_cls, "name", None)
        if stmt_declared_name and stmt_declared_name != stmt_cls.__name__:
            stmt_map[stmt_declared_name] = stmt_cls

    DIALECTS_LOOKUP[dialect.name] = (dialect, stmt_map)


def register_type(obj_type: type):
    RUNTIME_NAME2TYPE[obj_type.__name__] = obj_type


def runtime_register_encode(obj_type):
    def wrapper(func):
        if obj_type.__name__ in RUNTIME_ENCODE_LOOKUP:
            pass

        # TODO check func signature
        RUNTIME_ENCODE_LOOKUP[obj_type.__name__] = func
        return func

    return wrapper


def runtime_register_decode(obj_type):
    def wrapper(func):
        if obj_type.__name__ in RUNTIME_DECODE_LOOKUP:
            pass

        # TODO check func signature
        RUNTIME_DECODE_LOOKUP[obj_type.__name__] = func
        return func

    return wrapper


@dataclass
class DialectSerializer:
    def encode(self, obj: ir.Dialect | ir.DialectGroup | None):
        if isinstance(obj, ir.DialectGroup):
            return self.encode_dialect_group(obj)
        elif isinstance(obj, ir.Dialect):
            return self.encode_dialect(obj)
        else:
            raise ValueError(f"Unsupported dialect type {type(obj)} for encoding.")

    def decode(self, data: dict):
        if "kind" not in data:
            raise ValueError("Invalid dialect data for decoding.")

        match data["kind"]:
            case "dialect":
                return self.decode_dialect(data)
            case "dialect_group":
                return self.decode_dialect_group(data)
            case _:
                raise ValueError(
                    f"Unsupported dialect kind {data['kind']} for decoding."
                )

    def encode_dialect_group(self, group: ir.DialectGroup) -> dict:
        return {
            "kind": "dialect_group",
            "dialects": [self.encode_dialect(dialect) for dialect in group.data],
        }

    def decode_dialect_group(self, data: dict) -> ir.DialectGroup:
        if data.get("kind") != "dialect_group":
            raise ValueError("Not a dialect group data for decoding.")

        dialects = [
            self.decode_dialect(dialect_data) for dialect_data in data["dialects"]
        ]
        return ir.DialectGroup(dialects=dialects)

    def encode_dialect(self, obj: ir.Dialect):
        if obj.name not in DIALECTS_LOOKUP:
            raise ValueError(f"No registered dialect for {obj.name}. {DIALECTS_LOOKUP}")

        return {
            "kind": "dialect",
            "name": obj.name,
        }

    def decode_dialect(self, data: dict):
        if data.get("kind") != "dialect":
            raise ValueError("Not a dialect data for decoding.")

        name = data.get("name")
        if name not in DIALECTS_LOOKUP:
            raise ValueError(f"No registered dialect for name {name}.")

        return DIALECTS_LOOKUP[name][0]


@dataclass
class RuntimeSerializer:

    def encode(self, obj):
        if obj.__class__.__name__ not in RUNTIME_ENCODE_LOOKUP:
            raise ValueError(
                f"No registered encode method for {obj.__class__.__name__}. {RUNTIME_ENCODE_LOOKUP}"
            )

        return {
            "kind": "runtime",
            "style": obj.__class__.__name__,
            "data": RUNTIME_ENCODE_LOOKUP[obj.__class__.__name__](self, obj),
        }

    def decode(self, data):
        if data.get("kind") != "runtime":
            raise ValueError("Invalid runtime data for decoding.")

        style = data.get("style")
        if style not in RUNTIME_DECODE_LOOKUP:
            raise ValueError(f"No registered decode method for style {style}.")

        return RUNTIME_DECODE_LOOKUP[style](self, data.get("data"))


@dataclass
class TypeAttributeSerializer:
    def encode(self, obj: types.TypeAttribute):
        encode_content_method = getattr(self, f"_encode_{obj.__class__.__name__}", None)
        if not encode_content_method:
            raise ValueError(f"No encode method for {obj.__class__.__name__}")

        content_data = encode_content_method(obj)

        return {
            "kind": "type_attr",
            "style": obj.__class__.__name__,
            "data": content_data,
        }

    def decode(self, data: dict):
        if data.get("kind") != "type_attr":
            raise ValueError("Invalid type attribute data for decoding.")

        style = data.get("style")
        decode_method = getattr(self, f"_decode_{style}", None)
        if not decode_method:
            raise ValueError(f"No decode method for style {style}.")

        return decode_method(data.get("data"))

    def _encode_PyClass(self, obj: types.PyClass):
        if obj.typ.__name__ not in RUNTIME_NAME2TYPE:
            raise ValueError(
                f"No registered type for {obj.typ.__name__}. {RUNTIME_NAME2TYPE}"
            )

        return {
            "typ": obj.typ.__name__,
            "display_name": obj.display_name,
            "prefix": obj.prefix,
        }

    def _decode_PyClass(self, content_data: dict):

        typ_data = content_data["typ"]
        if typ_data not in RUNTIME_NAME2TYPE:
            raise ValueError(f"No registered type for {typ_data}.")

        pytype = RUNTIME_NAME2TYPE[typ_data]

        return types.PyClass(
            typ=pytype,
            display_name=content_data.get("display_name", ""),
            prefix=content_data.get("prefix", ""),
        )

    def _encode_AnyType(self, obj: types.AnyType):
        return dict()

    def _decode_AnyType(self, content_data: dict):
        return types.AnyType()

    def _encode_BottomType(self, obj: types.BottomType):
        return dict()

    def _decode_BottomType(self, content_data: dict):
        return types.BottomType()

    def _encode_TypeVar(self, obj: types.TypeVar):
        return {
            "varname": obj.varname,
            "bound": self.encode(obj.bound) if obj.bound else None,
        }

    def _decode_TypeVar(self, content_data: dict):
        bound = content_data.get("bound")
        if bound:
            bound = self.decode(bound)

        return types.TypeVar(name=content_data["varname"], bound=bound)

    def _encode_Vararg(self, obj: types.Vararg):
        return {"typ": self.encode(obj.typ)}

    def _decode_Vararg(self, content_data: dict):

        typ = self.decode(content_data["typ"])

        return types.Vararg(typ=typ)

    def _encode_Union(self, obj: types.Union):
        return {"types": [self.encode(typ) for typ in obj.types]}

    def _decode_Union(self, content_data: dict):
        types_list = content_data["types"]
        return types.Union(self.decode(typ) for typ in types_list)

    def _encode_Generic(self, obj: types.Generic):
        return {
            "body": self.encode(obj.body),
            "vars": [self.encode(arg) for arg in obj.vars],
            "vararg": self.encode(obj.vararg) if obj.vararg else None,
        }

    def _decode_Generic(self, content_data: dict):
        out = types.Generic.__new__(types.Generic)

        out.body = self.decode(content_data["body"])
        out.vars = tuple(self.decode(var) for var in content_data["vars"])
        vararg = content_data.get("vararg")
        if vararg:
            out.vararg = self.decode(vararg)

        return out

    def _encode_Literal(self, obj: types.Literal):
        return {
            "value": obj.data,  # note we assume this data can be serialized directly with json.
            "type": self.encode(obj.type) if obj.type else None,
        }

    def _decode_Literal(self, content_data: dict):
        value = content_data["value"]
        typ = content_data.get("type")
        if typ:
            typ = self.decode(typ)

        return types.Literal(data=value, datatype=typ)
