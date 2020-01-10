#ifndef __THRIFT_FFI_CBINDGEN_H__
#define __THRIFT_FFI_CBINDGEN_H__

/* Generated with cbindgen:0.12.1 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  uint64_t read_capacity;
  uint64_t write_capacity;
} MonocastClient;

typedef enum {
  Monocast,
} ClientRequest_Tag;

typedef struct {
  MonocastClient _0;
} Monocast_Body;

typedef struct {
  ClientRequest_Tag tag;
  union {
    Monocast_Body monocast;
  };
} ClientRequest;

typedef enum {
  Created,
  Failed,
} ClientCreationResult_Tag;

typedef struct {
  UserClientHandle *_0;
} Created_Body;

typedef struct {
  ClientCreationResult_Tag tag;
  union {
    Created_Body created;
  };
} ClientCreationResult;

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

void create_thrift_ffi_client(const ClientRequest *request, ClientCreationResult *result);

void destroy_thrift_ffi_client(UserClientHandle *handle);

void read_buffer_handle(UserClientHandle *handle, ThriftChunk chunk, ThriftReadResult *result);

void write_buffer_handle(UserClientHandle *handle, ThriftChunk chunk, ThriftWriteResult *result);

#endif /* __THRIFT_FFI_CBINDGEN_H__ */
