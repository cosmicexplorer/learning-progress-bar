#ifndef __THRIFT_FFI_CBINDGEN_H__
#define __THRIFT_FFI_CBINDGEN_H__

/* Generated with cbindgen:0.12.1 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  uint64_t _0;
} InternKey;

typedef struct {
  InternKey key;
} ThriftBufferHandle;

typedef enum {
  Created,
  Failed,
} ThriftFFIClientCreationResult_Tag;

typedef struct {
  ThriftBufferHandle _0;
} Created_Body;

typedef struct {
  ThriftFFIClientCreationResult_Tag tag;
  union {
    Created_Body created;
  };
} ThriftFFIClientCreationResult;

typedef struct {
  uint8_t *ptr;
  uint64_t len;
  uint64_t capacity;
} ThriftChunk;

typedef enum {
  Read,
  Failed,
} ThriftReadResult_Tag;

typedef struct {
  ThriftChunk _0;
} Read_Body;

typedef struct {
  ThriftReadResult_Tag tag;
  union {
    Read_Body read;
  };
} ThriftReadResult;

typedef enum {
  Written,
  Failed,
} ThriftWriteResult_Tag;

typedef struct {
  uint64_t _0;
} Written_Body;

typedef struct {
  ThriftWriteResult_Tag tag;
  union {
    Written_Body written;
  };
} ThriftWriteResult;

ThriftFFIClientCreationResult make_buffer_handle(uintptr_t capacity);

ThriftReadResult read_buffer_handle(ThriftBufferHandle handle, ThriftChunk chunk);

ThriftWriteResult write_buffer_handle(ThriftBufferHandle handle, ThriftChunk chunk);

#endif /* __THRIFT_FFI_CBINDGEN_H__ */
