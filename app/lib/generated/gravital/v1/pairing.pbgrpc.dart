// This is a generated file - do not edit.
//
// Generated from gravital/v1/pairing.proto.

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

import 'pairing.pb.dart' as $0;

export 'pairing.pb.dart';

@$pb.GrpcServiceName('gravital.v1.PairingService')
class PairingServiceClient extends $grpc.Client {
  /// The hostname for this service.
  static const $core.String defaultHost = '';

  /// OAuth scopes needed for the client.
  static const $core.List<$core.String> oauthScopes = [
    '',
  ];

  PairingServiceClient(super.channel, {super.options, super.interceptors});

  /// / El host publica su oferta (p.ej. en el relay para anclaje NAT).
  /// / Devuelve la oferta canónica aceptada (o error en caso de rechazo).
  $grpc.ResponseFuture<$0.PairingOffer> publishOffer(
    $0.PairingOffer request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$publishOffer, request, options: options);
  }

  /// / El peer recupera la oferta por URI o ID y confirma.
  $grpc.ResponseFuture<$0.PairingOffer> resolveOffer(
    $0.PairingOffer request, {
    $grpc.CallOptions? options,
  }) {
    return $createUnaryCall(_$resolveOffer, request, options: options);
  }

  // method descriptors

  static final _$publishOffer =
      $grpc.ClientMethod<$0.PairingOffer, $0.PairingOffer>(
          '/gravital.v1.PairingService/PublishOffer',
          ($0.PairingOffer value) => value.writeToBuffer(),
          $0.PairingOffer.fromBuffer);
  static final _$resolveOffer =
      $grpc.ClientMethod<$0.PairingOffer, $0.PairingOffer>(
          '/gravital.v1.PairingService/ResolveOffer',
          ($0.PairingOffer value) => value.writeToBuffer(),
          $0.PairingOffer.fromBuffer);
}

@$pb.GrpcServiceName('gravital.v1.PairingService')
abstract class PairingServiceBase extends $grpc.Service {
  $core.String get $name => 'gravital.v1.PairingService';

  PairingServiceBase() {
    $addMethod($grpc.ServiceMethod<$0.PairingOffer, $0.PairingOffer>(
        'PublishOffer',
        publishOffer_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.PairingOffer.fromBuffer(value),
        ($0.PairingOffer value) => value.writeToBuffer()));
    $addMethod($grpc.ServiceMethod<$0.PairingOffer, $0.PairingOffer>(
        'ResolveOffer',
        resolveOffer_Pre,
        false,
        false,
        ($core.List<$core.int> value) => $0.PairingOffer.fromBuffer(value),
        ($0.PairingOffer value) => value.writeToBuffer()));
  }

  $async.Future<$0.PairingOffer> publishOffer_Pre(
      $grpc.ServiceCall $call, $async.Future<$0.PairingOffer> $request) async {
    return publishOffer($call, await $request);
  }

  $async.Future<$0.PairingOffer> publishOffer(
      $grpc.ServiceCall call, $0.PairingOffer request);

  $async.Future<$0.PairingOffer> resolveOffer_Pre(
      $grpc.ServiceCall $call, $async.Future<$0.PairingOffer> $request) async {
    return resolveOffer($call, await $request);
  }

  $async.Future<$0.PairingOffer> resolveOffer(
      $grpc.ServiceCall call, $0.PairingOffer request);
}
