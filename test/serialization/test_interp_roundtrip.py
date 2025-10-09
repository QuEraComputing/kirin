from kirin.prelude import basic
from kirin.dialects import ilist
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.base.deserializer import Deserializer
import inspect, importlib

def test_round_trip_with_interp():

    @basic
    def my_kernel1(x: int):
        return (x, x + 1, 3)


    @basic
    def my_kernel2(y: int):
        return my_kernel1(y) * 10

    # serializer = Serializer()
    # deserializer = Deserializer()
    # encoded = serializer.encode(my_kernel2)
    
    # json_serializer = JSONSerializer()
    # json_encoded = json_serializer.encode(encoded)

    # py_module_name = inspect.getmodule(ilist).__name__
    # print(py_module_name)

    # print(ilist.dialect)

    # print(inspect.getsourcefile(ilist.dialect))
    # # find:
    
    # new_ilist_mod = importlib.import_module(py_module_name)

    

    print(basic.data)

    ##decoded = deserializer.decode(encoded)
test_round_trip_with_interp()