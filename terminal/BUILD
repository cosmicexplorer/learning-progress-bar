cargo_subproject(
  name='terminal-wrapper',
  cargo_output_file='libterminal_wrapper.dylib',
  sources=rglobs('*.toml', '*.rs'),
  dependencies=[
    '//:rust-toolchain',
    ':streaming-interface',
  ],
)

python_thrift_library(
  name='streaming-interface',
  sources=globs('src/*.thrift'),
)

python_library(
  name='event-loop',
  dependencies=[
    ':streaming-interface',
  ],
)
