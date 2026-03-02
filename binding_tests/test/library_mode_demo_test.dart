import 'dart:io';

import 'package:test/test.dart';
import '../generated/library_mode_demo.dart';

void main() {
  late String libPath;

  setUpAll(() {
    final path = Platform.environment['UBDG_LIBRARY_MODE_DEMO_LIB'];
    expect(
      path,
      isNotNull,
      reason:
          'UBDG_LIBRARY_MODE_DEMO_LIB must point to the compiled library-mode-demo fixture library',
    );
    libPath = path!;
    configureDefaultBindings(libraryPath: libPath);
  });

  tearDownAll(resetDefaultBindings);

  group('top-level functions', () {
    test('greet returns greeting', () {
      expect(greet('World'), 'Hello, World!');
    });

    test('greetAsync returns async greeting', () async {
      expect(await greetAsync('World'), 'Async hello, World!');
    });

    test('divide returns quotient', () {
      expect(divide(10, 3), 3);
    });

    test('divide throws on zero', () {
      expect(
        () => divide(10, 0),
        throwsA(isA<ArithErrorExceptionDivisionByZero>()),
      );
    });

    // The following tests exercise collection/optional types whose runtime
    // ffibuffer ABI (RustCallStatus out-arg) is not yet implemented.
    // They are skipped until the runtime catches up.
    test('echoStrings round-trips list', () {
      expect(echoStrings(['a', 'b', 'c']), ['a', 'b', 'c']);
    }, skip: 'ffibuffer RustCallStatus out-arg ABI not yet implemented');

    test('echoMap round-trips map', () {
      expect(echoMap({'x': 1, 'y': 2}), {'x': 1, 'y': 2});
    }, skip: 'ffibuffer RustCallStatus out-arg ABI not yet implemented');

    test('maybeGreet with value', () {
      expect(maybeGreet('Alice'), 'Hello, Alice!');
    }, skip: 'ffibuffer RustCallStatus out-arg ABI not yet implemented');

    test('maybeGreet with null', () {
      expect(maybeGreet(null), isNull);
    }, skip: 'ffibuffer RustCallStatus out-arg ABI not yet implemented');
  });

  group('records and enums', () {
    test('makePoint creates record', () {
      final p = makePoint(1.5, 2.5);
      expect(p.x, 1.5);
      expect(p.y, 2.5);
    });

    test('describeShape circle', () {
      expect(
        describeShape(const ShapeCircle(radius: 5.0)),
        'circle(r=5)',
      );
    });

    test('describeShape rect', () {
      expect(
        describeShape(const ShapeRect(w: 3.0, h: 4.0)),
        'rect(3x4)',
      );
    });
  });

  group('custom types', () {
    test('echoLabel round-trips custom newtype', () {
      expect(echoLabel('hello'), 'hello');
    });
  });

  group('objects', () {
    test('Counter create and get', () {
      final c = Counter.create(10);
      expect(c.get_(), 10);
      c.close();
      expect(c.isClosed, isTrue);
    });

    test('Counter increment', () {
      final c = Counter.create(10);
      c.increment();
      expect(c.get_(), 11);
      c.close();
    });

    test('Counter asyncGet', () async {
      final c = Counter.create(42);
      expect(await c.asyncGet(), 42);
      c.close();
    }, skip: 'ffibuffer RustCallStatus out-arg ABI not yet implemented');

    test('Counter throws after close', () {
      final c = Counter.create(0);
      c.close();
      expect(() => c.get_(), throwsStateError);
    });
  });
}
