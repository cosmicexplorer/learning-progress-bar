python_library(
  dependencies=[
    '3rdparty/python:setuptools',
    'terminal:terminal-wrapper',
    'terminal:streaming-interface',
  ],
)

python_binary(
  name='wow',
  source='wow.py',
  dependencies=[
    ':inference',
    '3rdparty/python:cffi',
  ],
  zip_safe=False,
)
