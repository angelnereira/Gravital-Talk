/// Panel con QR del room code (GEUI).
library;

import 'package:flutter/material.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../tokens.dart';

/// Panel con QR del room code.
class QrPanel extends StatelessWidget {
  const QrPanel({super.key, required this.code, this.caption});

  final String code;
  final String? caption;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      children: [
        Container(
          padding: const EdgeInsets.all(NSpace.sm),
          decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(NShape.large),
          ),
          child: QrImageView(
            data: code,
            size: 150,
            backgroundColor: Colors.white,
          ),
        ),
        const SizedBox(height: NSpace.sm),
        SelectableText(
          code,
          style: theme.textTheme.titleLarge?.copyWith(
            fontWeight: FontWeight.w800,
            letterSpacing: 2,
          ),
        ),
        if (caption != null)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(caption!, style: theme.textTheme.bodySmall),
          ),
      ],
    );
  }
}
