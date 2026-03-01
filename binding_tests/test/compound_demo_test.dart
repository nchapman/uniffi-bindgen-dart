import 'dart:io';
import 'dart:typed_data';

import 'package:test/test.dart';
import '../generated/compound_demo.dart';

void main() {
  setUp(() {
    final path = Platform.environment['UBDG_COMPOUND_DEMO_LIB'];
    expect(
      path,
      isNotNull,
      reason:
          'UBDG_COMPOUND_DEMO_LIB must point to the compiled compound-demo fixture library',
    );
    configureDefaultBindings(libraryPath: path!);
  });

  tearDown(() {
    resetDefaultBindings();
  });

  test('generated bindings file exists', () {
    final generated = File('generated/compound_demo.dart');
    expect(generated.existsSync(), isTrue);
  });

  group('counts', () {
    test('round-trip with total', () {
      final result = counts({'a': 2, 'b': 3});
      expect(result['a'], 2);
      expect(result['b'], 3);
      expect(result['total'], 5);
    });

    test('empty map', () {
      final result = counts(<String, int>{});
      expect(result['total'], 0);
    });
  });

  group('maybeName', () {
    test('with value', () {
      expect(maybeName('hello'), 'hello');
    });

    test('with null', () {
      expect(maybeName(null), isNull);
    });
  });

  group('chunk', () {
    test('round-trip', () {
      final input = [
        Uint8List.fromList([1, 2, 3]),
        Uint8List(0),
        Uint8List.fromList([4, 5]),
      ];
      final result = chunk(input);
      expect(result.length, 3);
      expect(result[0], Uint8List.fromList([1, 2, 3]));
      expect(result[1], Uint8List(0));
      expect(result[2], Uint8List.fromList([4, 5]));
    });

    test('empty list', () {
      expect(chunk(<Uint8List>[]), <Uint8List>[]);
    });
  });
}
