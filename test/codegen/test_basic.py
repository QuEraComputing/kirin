from kirin.prelude import basic


@basic
def foo(x: int):  # type: ignore
    def goo(y: int):
        return x + y

    return goo


# def test_basic():
#     d = DictGen(basic).emit(foo)
#     assert d == {
#         "globals": {},
#         "methods": {},
#         "entry": "foo",
#         "foo": {
#             "name": "foo",
#             "args": {
#                 "names": ["#self#", "x"],
#                 "types": [{"name": "class", "value": "class.int"}],
#             },
#             "body": {
#                 "dialect": "func",
#                 "type": "Function",
#                 "args": [],
#                 "results": [],
#                 "successors": [],
#                 "regions": [
#                     {
#                         "type": "ir.region",
#                         "blocks": [
#                             {
#                                 "type": "ir.block",
#                                 "id": 0,
#                                 "stmts": [
#                                     {
#                                         "dialect": "func",
#                                         "type": "Lambda",
#                                         "args": [0],
#                                         "results": [1],
#                                         "successors": [],
#                                         "regions": [
#                                             {
#                                                 "type": "ir.region",
#                                                 "blocks": [
#                                                     {
#                                                         "type": "ir.block",
#                                                         "id": 1,
#                                                         "stmts": [
#                                                             {
#                                                                 "dialect": "func",
#                                                                 "type": "GetField",
#                                                                 "args": [2],
#                                                                 "results": [3],
#                                                                 "successors": [],
#                                                                 "regions": [],
#                                                                 "properties": {
#                                                                     "field": {
#                                                                         "name": "PyAttr",
#                                                                         "data": "0",
#                                                                         "type": {
#                                                                             "name": "class",
#                                                                             "value": "class.int",
#                                                                         },
#                                                                     }
#                                                                 },
#                                                                 "attributes": {},
#                                                             },
#                                                             {
#                                                                 "dialect": "py.stmts",
#                                                                 "type": "Add",
#                                                                 "args": [3, 4],
#                                                                 "results": [5],
#                                                                 "successors": [],
#                                                                 "regions": [],
#                                                                 "properties": {},
#                                                                 "attributes": {},
#                                                             },
#                                                             {
#                                                                 "dialect": "func",
#                                                                 "type": "Return",
#                                                                 "args": [5],
#                                                                 "results": [],
#                                                                 "successors": [],
#                                                                 "regions": [],
#                                                                 "properties": {},
#                                                                 "attributes": {},
#                                                             },
#                                                         ],
#                                                     }
#                                                 ],
#                                             }
#                                         ],
#                                         "properties": {
#                                             "sym_name": {
#                                                 "name": "PyAttr",
#                                                 "data": "'goo'",
#                                                 "type": {
#                                                     "name": "class",
#                                                     "value": "class.str",
#                                                 },
#                                             }
#                                         },
#                                         "attributes": {
#                                             "signature": {
#                                                 "name": "Signature",
#                                                 "inputs": [
#                                                     {
#                                                         "name": "class",
#                                                         "value": "class.int",
#                                                     }
#                                                 ],
#                                                 "output": {"name": "Any"},
#                                             }
#                                         },
#                                     },
#                                     {
#                                         "dialect": "func",
#                                         "type": "Return",
#                                         "args": [1],
#                                         "results": [],
#                                         "successors": [],
#                                         "regions": [],
#                                         "properties": {},
#                                         "attributes": {},
#                                     },
#                                 ],
#                             }
#                         ],
#                     }
#                 ],
#                 "properties": {
#                     "sym_name": {
#                         "name": "PyAttr",
#                         "data": "'foo'",
#                         "type": {"name": "class", "value": "class.str"},
#                     }
#                 },
#                 "attributes": {
#                     "signature": {
#                         "name": "Signature",
#                         "inputs": [{"name": "class", "value": "class.int"}],
#                         "output": {"name": "Any"},
#                     }
#                 },
#             },
#         },
#     }
