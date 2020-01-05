from setuptools import find_packages, setup


setup(
  name='thrift_ffi',
  version='0.0.1',,
  packages=find_packages(),
  data_files=[('', 'libthrift_ffi.so')],
)
