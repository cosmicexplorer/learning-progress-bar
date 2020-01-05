cargo_subproject(
  name='terminal-rust',
  cargo_output_file='libterminal_wrapper.dylib',
  sources=rglobs('*.toml', '*.rs'),
  cargo_subprojects=[
    'thrift_ffi',
  ],
  dependencies=[
    '//:rust-toolchain',
    'terminal/thrift:rust',
  ],
)

python_dist(
  name='terminal-python',
  sources=rglobs('*.py', '*.h'),
)

python_library(
  name='event-loop',
  dependencies=[
    ':streaming-interface',
  ],
)
