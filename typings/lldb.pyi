
from typing import Any, List, Optional

frame: SBFrame | None
thread: SBThread | None
process: SBProcess | None
target: SBTarget | None
debugger: SBDebugger | None


def command(command_name=None, doc=None): ...


class SBType:
    ...


class SBValue:
    def IsValid(self) -> bool: ...
    def MightHaveChildren(self) -> bool: ...
    def GetNumChildren(self) -> int: ...
    def GetChildAtIndex(self, index: int) -> SBValue: ...
    def GetIndexOfChildWithName(self, name: str) -> int: ...
    def SetPreferSyntheticValue(self, on: bool): ...
    def SetFormat(self, fmt: int): ...


class SBError:
    def Success(self) -> bool: ...
    def GetCString(self) -> str: ...
    def SetErrorString(self, s: str): ...


class SBData:
    def SetData(self, err: SBError, buffer: Any, order: int, size: int): ...
    @staticmethod
    def CreateDataFromCString(order: int, size: int, value: Any): ...


class SBTypeSynthetic:
    @staticmethod
    def CreateWithClassName(cls_name: str): ...


class SBTypeSummary:
    @staticmethod
    def CreateWithFunctionName(fn_name: str): ...


class SBTypeNameSpecifier:
    def __init__(self, name: str, is_regex: bool): ...


class SBExecutionContext:
    def GetTarget(self) -> SBTarget: ...
    def GetFrame(self) -> SBFrame: ...


class SBModule:
    ...


class SBFrame:
    ...


class SBThread:
    ...


class SBProcess:
    ...


class SBTarget:
    modules: List[SBModule]
    def GetAddressByteSize(self) -> int: ...
    def GetDebugger(self) -> SBDebugger: ...


class SBDebugger:
    @staticmethod
    def Create() -> SBDebugger: ...
    @staticmethod
    def Destroy(debugger: SBDebugger): ...
    @staticmethod
    def SetInternalVariable(name: str, value: str, instance_name: str) -> SBError: ...
    def GetInstanceName(self) -> str: ...
    def GetID(self) -> int: ...
    def HandleCommand(self, command: str): ...

    def CreateTarget(self, filename: str, target_triple: Optional[str], platform_name: Optional[str],
                     add_dependent_modules: bool, error: SBError) -> SBTarget: ...


eTypeOptionCascade: int

eBasicTypeInvalid: int
eBasicTypeVoid: int
eBasicTypeChar: int
eBasicTypeSignedChar: int
eBasicTypeUnsignedChar: int
eBasicTypeWChar: int
eBasicTypeSignedWChar: int
eBasicTypeUnsignedWChar: int
eBasicTypeChar16: int
eBasicTypeChar32: int
eBasicTypeShort: int
eBasicTypeUnsignedShort: int
eBasicTypeInt: int
eBasicTypeUnsignedInt: int
eBasicTypeLong: int
eBasicTypeUnsignedLong: int
eBasicTypeLongLong: int
eBasicTypeUnsignedLongLong: int
eBasicTypeInt128: int
eBasicTypeUnsignedInt128: int
eBasicTypeBool: int
eBasicTypeHalf: int
eBasicTypeFloat: int
eBasicTypeDouble: int
eBasicTypeLongDouble: int
eBasicTypeFloatComplex: int
eBasicTypeDoubleComplex: int
eBasicTypeLongDoubleComplex: int
eBasicTypeObjCID: int
eBasicTypeObjCClass: int
eBasicTypeObjCSel: int
eBasicTypeNullPtr: int

eFormatChar: int

eTypeClassStruct: int

eValueTypeVariableGlobal: int
eValueTypeVariableStatic: int
eValueTypeRegister: int
eValueTypeConstResult: int

eReturnStatusSuccessFinishResult: int
