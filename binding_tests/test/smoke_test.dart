import 'dart:io';

import 'package:test/test.dart';
import '../generated/simple_fns.dart';

void main() {
  test('binding test scaffold is wired', () {
    expect(true, isTrue);
  });

  test('generated bindings file exists', () {
    final generated = File('generated/simple_fns.dart');
    expect(generated.existsSync(), isTrue);
  });

  test('generated bindings include expected symbols', () {
    final generated = File('generated/simple_fns.dart');
    final contents = generated.readAsStringSync();
    expect(contents, contains('library simple_fns;'));
    expect(contents, contains('class SimpleFnsBindings {'));
    expect(contents, contains("libraryName = 'uniffi_simple_fns';"));
    expect(contents, contains('int add(int left, int right) {'));
    expect(contents, contains('int negate(int value) {'));
    expect(contents, contains('String greet(String name) {'));
    expect(contents, contains('bool isEven(int value) {'));
    expect(contents, contains('double scale(double value, double factor) {'));
    expect(contents, contains('void tick() {'));
    expect(contents, contains('int currentTick() {'));
    expect(contents, contains('late final int Function(int left, int right) _add ='));
    expect(contents, contains('late final bool Function(int value) _isEven ='));
    expect(contents, contains('ffi.Pointer<Utf8> nameNative = name.toNativeUtf8();'));
    expect(contents, contains('_rustStringFree(resultPtr);'));
    expect(
      contents,
      contains(
        'late final double Function(double value, double factor) _scale =',
      ),
    );
    expect(contents, contains('late final void Function() _tick ='));
    expect(contents, contains('void configureDefaultBindings('));
    expect(contents, contains('return _bindings().add(left, right);'));
  });

  test('runtime ffi binding can call native exports', () {
    final libPath = Platform.environment['UBDG_SIMPLE_FNS_LIB'];
    expect(
      libPath,
      isNotNull,
      reason:
          'UBDG_SIMPLE_FNS_LIB must point to the compiled simple-fns fixture library',
    );

    final bindings = SimpleFnsBindings(libraryPath: libPath);
    expect(bindings.add(20, 22), 42);
    expect(bindings.negate(7), -7);
    expect(bindings.greet('dart'), 'hello, dart');
    expect(bindings.isEven(8), isTrue);
    expect(bindings.isEven(9), isFalse);
    expect(bindings.scale(2.5, 4.0), closeTo(10.0, 0.000001));
    final before = bindings.currentTick();
    bindings.tick();
    expect(bindings.currentTick(), before + 1);

    configureDefaultBindings(libraryPath: libPath);
    expect(add(1, 2), 3);
    expect(negate(5), -5);
    expect(greet('ffi'), 'hello, ffi');
    expect(isEven(10), isTrue);
    expect(isEven(11), isFalse);
    expect(scale(1.5, 3.0), closeTo(4.5, 0.000001));
    final globalBefore = currentTick();
    tick();
    expect(currentTick(), globalBefore + 1);
    resetDefaultBindings();
  });
}
