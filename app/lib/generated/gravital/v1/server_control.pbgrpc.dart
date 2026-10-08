// This is a generated file - do not edit.
//
// Generated from gravital/v1/server_control.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:async' as $async;
import 'dart:core' as $core;

import 'package:grpc/service_api.dart' as $grpc;
import 'package:protobuf/protobuf.dart' as $pb;

import 'server_control.pb.dart' as $0;

export 'server_control.pb.dart';

@$pb.GrpcServiceName('gravital.v1.ServerControl')
class ServerControlClient extends $grpc.Client {
  /// The hostname for this service.
  static const $core.String defaultHost = '';

  /// OAuth scopes needed for the client.
  static const $core.List<$core.String> oauthScopes = [
    '',
  ];

  ServerControlClient(super.channel, {super.options, super.interceptors});

  /// Información general del servidor (versión, límites, carga).
  $grpc.ResponseFuture<$0.ServerInfoResponse> getServerInfo(
    $0.ServerInfoRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$getServerInfo, request, options: options);
  }

  /// Health + métricas agregadas (equivalente a /healthz y /metrics).
  $grpc.ResponseFuture<$0.ServerHealthResponse> getServerHealth(
    $0.ServerHealthRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$getServerHealth, request, options: options);
  }

  /// Rerserva una sala nueva (host).
  $grpc.ResponseFuture<$0.CreateRoomResponse> createRoom(
    $0.CreateRoomRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$createRoom, request, options: options);
  }

  /// Resuelve un código de sala.
  $grpc.ResponseFuture<$0.GetRoomResponse> getRoom(
    $0.GetRoomRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$getRoom, request, options: options);
  }

  /// Lista salas activas.
  $grpc.ResponseFuture<$0.ListRoomsResponse> listRooms(
    $0.ListRoomsRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$listRooms, request, options: options);
  }

  /// Elimina una sala (.eviction administrativa).
  $grpc.ResponseFuture<$0.DeleteRoomResponse> deleteRoom(
    $0.DeleteRoomRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$deleteRoom, request, options: options);
  }

  /// Stream de eventos de una sala (join/leave/floor).
  $grpc.ResponseStream<$0.RoomEvent> watchRoom(
    $0.WatchRoomRequest request, {
    $grpc.CallOptions? options,
  }) {
    return $createStreamingCall(
        _$watchRoom, $async.Stream.fromIterable([request]),
        options: options);
  }

  // method descriptors

  static final _$getServerInfo =
      $grpc.ClientMethod<$0.ServerInfoRequest, $0.ServerInfoResponse>(
          '/gravital.v1.ServerControl/GetServerInfo',
          ($0.ServerInfoRequest value) => value.writeToBuffer(),
          $0.ServerInfoResponse.fromBuffer);
  static final _$getServerHealth =
      $grpc.ClientMethod<$0.ServerHealthRequest, $0.ServerHealthResponse>(
          '/gravital.v1.ServerControl/GetServerHealth',
          ($0.ServerHealthRequest value) => value.writeToBuffer(),
          $0.ServerHealthResponse.fromBuffer);
  static final _$createRoom =
      $grpc.ClientMethod<$0.CreateRoomRequest, $0.CreateRoomResponse>(
          '/gravital.v1.ServerControl/CreateRoom',
          ($0.CreateRoomRequest value) => value.writeToBuffer(),
          $0.CreateRoomResponse.fromBuffer);
  static final _$getRoom =
      $grpc.ClientMethod<$0.GetRoomRequest, $0.GetRoomResponse>(
          '/gravital.v1.ServerControl/GetRoom',
          ($0.GetRoomRequest value) => value.writeToBuffer(),
          $0.GetRoomResponse.fromBuffer);
  static final _$listRooms =
      $grpc.ClientMethod<$0.ListRoomsRequest, $0.ListRoomsResponse>(
          '/gravital.v1.ServerControl/ListRooms',
          ($0.ListRoomsRequest value) => value.writeToBuffer(),
          $0.ListRoomsResponse.fromBuffer);
  static final _$deleteRoom =
      $grpc.ClientMethod<$0.DeleteRoomRequest, $0.DeleteRoomResponse>(
          '/gravital.v1.ServerControl/DeleteRoom',
          ($0.DeleteRoomRequest value) => value.writeToBuffer(),
          $0.DeleteRoomResponse.fromBuffer);
  static final _$watchRoom =
      $grpc.ClientMethod<$0.WatchRoomRequest, $0.RoomEvent>(
          '/gravital.v1.ServerControl/WatchRoom',
          ($0.WatchRoomRequest value) => value.writeToBuffer(),
          $0.RoomEvent.fromBuffer);
}

@$pb.GrpcServiceName('gravital.v1.ServerControl')
abstract class ServerControlServiceBase extends $grpc.Service {
  $core.String get $name => 'gravital.v1.ServerControl';

  ServerControlServiceBase() {
    $addMethod($grpc.ServiceMethod<$0.ServerInfoRequest, $0.ServerInfoResponse>(
        'GetServerInfo',
        getServerInfo_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.ServerInfoRequest.fromBuffer(value),
        ($0.ServerInfoResponse value) => value.writeToBuffer()));
    $addMethod(
        $grpc.ServiceMethod<$0.ServerHealthRequest, $0.ServerHealthResponse>(
            'GetServerHealth',
            getServerHealth_Pre,
            false,
            false,
            ($core.List<$core.int> value) =>
                $0.ServerHealthRequest.fromBuffer(value),
            ($0.ServerHealthResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.CreateRoomRequest, $0.CreateRoomResponse>(
        'CreateRoom',
        createRoom_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.CreateRoomRequest.fromBuffer(value),
        ($0.CreateRoomResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.GetRoomRequest, $0.GetRoomResponse>(
        'GetRoom',
        getRoom_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.GetRoomRequest.fromBuffer(value),
        ($0.GetRoomResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.ListRoomsRequest, $0.ListRoomsResponse>(
        'ListRooms',
        listRooms_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.ListRoomsRequest.fromBuffer(value),
        ($0.ListRoomsResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.DeleteRoomRequest, $0.DeleteRoomResponse>(
        'DeleteRoom',
        deleteRoom_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.DeleteRoomRequest.fromBuffer(value),
        ($0.DeleteRoomResponse value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.WatchRoomRequest, $0.RoomEvent>(
        'WatchRoom',
        watchRoom_Pre,
        false,
        true,
        ($core.List<$core.int> value) => $0.WatchRoomRequest.fromBuffer(value),
        ($0.RoomEvent value) => value.writeToBuffer()));
  }

  $async.Future<$0.ServerInfoResponse> getServerInfo_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.ServerInfoRequest> $request) async {
    return getServerInfo($call, await $request);
  }

  $async.Future<$0.ServerInfoResponse> getServerInfo(
      $grpc.ServiceCall call, $0.ServerInfoRequest request);

  $async.Future<$0.ServerHealthResponse> getServerHealth_Pre(
      $grpc.ServiceCall $call,
      $async.Future<$0.ServerHealthRequest> $request) async {
    return getServerHealth($call, await $request);
  }

  $async.Future<$0.ServerHealthResponse> getServerHealth(
      $grpc.ServiceCall call, $0.ServerHealthRequest request);

  $async.Future<$0.CreateRoomResponse> createRoom_Pre($grpc.ServiceCall $call,
      $async.Future<$0.CreateRoomRequest> $request) async {
    return createRoom($call, await $request);
  }

  $async.Future<$0.CreateRoomResponse> createRoom(
      $grpc.ServiceCall call, $0.CreateRoomRequest request);

  $async.Future<$0.GetRoomResponse> getRoom_Pre($grpc.ServiceCall $call,
      $async.Future<$0.GetRoomRequest> $request) async {
    return getRoom($call, await $request);
  }

  $async.Future<$0.GetRoomResponse> getRoom(
      $grpc.ServiceCall call, $0.GetRoomRequest request);

  $async.Future<$0.ListRoomsResponse> listRooms_Pre($grpc.ServiceCall $call,
      $async.Future<$0.ListRoomsRequest> $request) async {
    return listRooms($call, await $request);
  }

  $async.Future<$0.ListRoomsResponse> listRooms(
      $grpc.ServiceCall call, $0.ListRoomsRequest request);

  $async.Future<$0.DeleteRoomResponse> deleteRoom_Pre($grpc.ServiceCall $call,
      $async.Future<$0.DeleteRoomRequest> $request) async {
    return deleteRoom($call, await $request);
  }

  $async.Future<$0.DeleteRoomResponse> deleteRoom(
      $grpc.ServiceCall call, $0.DeleteRoomRequest request);

  $async.Stream<$0.RoomEvent> watchRoom_Pre($grpc.ServiceCall $call,
      $async.Future<$0.WatchRoomRequest> $request) async* {
    yield* watchRoom($call, await $request);
  }

  $async.Stream<$0.RoomEvent> watchRoom(
      $grpc.ServiceCall call, $0.WatchRoomRequest request);
}
