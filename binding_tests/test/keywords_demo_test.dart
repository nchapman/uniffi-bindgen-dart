import 'dart:io';

import 'package:test/test.dart';
import '../generated/keywords_demo.dart';

void main() {
  late String libPath;

  setUp(() {
    final path = Platform.environment['UBDG_KEYWORDS_DEMO_LIB'];
    expect(
      path,
      isNotNull,
      reason:
          'UBDG_KEYWORDS_DEMO_LIB must point to the compiled keywords-demo fixture library',
    );
    libPath = path!;
    configureDefaultBindings(libraryPath: libPath);
  });

  tearDown(() {
    resetDefaultBindings();
  });

  test('generated bindings file exists', () {
    final generated = File('generated/keywords_demo.dart');
    expect(generated.existsSync(), isTrue);
  });

  group('escaped top-level functions', () {
    test('class_ doubles the value', () {
      expect(class_(5), 10);
      expect(class_(0), 0);
    });

    test('is_ negates a boolean', () {
      expect(is_(true), isFalse);
      expect(is_(false), isTrue);
    });

    test('return_ prepends return:', () {
      expect(return_('hello'), 'return:hello');
      expect(return_(''), 'return:');
    });
  });

  group('Super object (escaped keyword)', () {
    test('create and call class_ method', () {
      final obj = Super.create();
      expect(obj.class_('input'), 'super:input');
      obj.close();
    });

    test('return_ method returns handle value', () {
      final obj = Super.create();
      expect(obj.return_(), isA<int>());
      obj.close();
    });

    test('close prevents further use', () {
      final obj = Super.create();
      obj.close();
      expect(() => obj.class_('x'), throwsA(isA<StateError>()));
    });
  });

  group('escaped type codecs', () {
    test('Async enum round-trip', () {
      for (final value in Async.values) {
        expect(AsyncFfiCodec.decode(AsyncFfiCodec.encode(value)), value);
      }
    });

    test('Throw sealed class round-trip', () {
      final catchVal = ThrowCatch();
      final decoded = ThrowFfiCodec.decode(ThrowFfiCodec.encode(catchVal));
      expect(decoded, isA<ThrowCatch>());

      final rethrowVal = ThrowRethrow();
      final decodedRethrow =
          ThrowFfiCodec.decode(ThrowFfiCodec.encode(rethrowVal));
      expect(decodedRethrow, isA<ThrowRethrow>());
    });

    test('ThrowException round-trip', () {
      final catchErr = ThrowExceptionCatch();
      final decoded =
          ThrowExceptionFfiCodec.decode(ThrowExceptionFfiCodec.encode(catchErr));
      expect(decoded, isA<ThrowExceptionCatch>());

      final rethrowErr = ThrowExceptionRethrow();
      final decodedRethrow = ThrowExceptionFfiCodec.decode(
          ThrowExceptionFfiCodec.encode(rethrowErr));
      expect(decodedRethrow, isA<ThrowExceptionRethrow>());
    });
  });

  group('Break record (escaped fields)', () {
    test('construction and toJson round-trip', () {
      final b = Break(class_: 'hello', return_: 42, is_: true);
      expect(b.class_, 'hello');
      expect(b.return_, 42);
      expect(b.is_, isTrue);

      final json = b.toJson();
      final restored = Break.fromJson(json);
      expect(restored.class_, 'hello');
      expect(restored.return_, 42);
      expect(restored.is_, isTrue);
    });

    test('copyWith', () {
      final b = Break(class_: 'orig', return_: 1, is_: false);
      final updated = b.copyWith(class_: 'new');
      expect(updated.class_, 'new');
      expect(updated.return_, 1);
    });
  });
}
