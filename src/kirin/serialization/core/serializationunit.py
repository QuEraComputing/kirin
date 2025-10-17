class SerializationUnit:
    kind: str
    module_name: str
    class_name: str
    data: dict

    def __init__(self, kind: str, module_name: str, class_name: str, data: dict):
        self.kind = kind
        self.module_name = module_name
        self.class_name = class_name
        self.data = data
