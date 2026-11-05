from importlib import import_module

module = import_module("webcodex.browser_bridge")
print(f"PASS: browser bridge imported from {module.__file__}")
