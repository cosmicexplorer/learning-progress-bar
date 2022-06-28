# from pathlib import Path
# from typing import Union

# from pkg_resources import get_provider, DefaultProvider, ZipProvider


# def get_provider_for_module(module_name: str) -> Union[DefaultProvider, ZipProvider]:
#   provider = get_provider(module_name)
#   if not isinstance(provider, DefaultProvider):
#     mod = __import__(module_Name, fromlist=['ignore'])
#     provider = ZipProvider(mod)
#   return provider


# def get_resource_string_from_module(module_name: str, rel_path: Path) -> bytes:
#   provider = get_provider_for_module(module_name)
#   return provider.get_resource_string(module_name, str(rel_path))
