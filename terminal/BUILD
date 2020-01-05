cargo_subproject(
  name='terminal-rust',
  cargo_output_file='libterminal_wrapper.dylib',
  sources=rglobs('*'),
  # sources=rglobs('*.toml', '*.rs',
  #                exclude=[zglobs('target/**/*')]),
  cargo_subprojects=[
    'thrift_ffi',
  ],
  dependencies=[
    '//:rust-toolchain',
    'terminal/thrift:rust',
  ],
)

python_library(
  name='terminal-python',
  dependencies=[
    '3rdparty/python:cffi',
    '3rdparty/python:pants',
    '3rdparty/python:thrift',
    '3rdparty/python:setuptools-floating',
    'thrift_ffi',
  ],
)

python_binary(
  name='bin',
  entry_point='event_loop:main',
  dependencies=[
    ':terminal-python',
  ],
)
