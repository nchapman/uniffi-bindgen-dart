import 'dart:io';
import 'dart:typed_data';

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
    expect(contents, contains('DateTime addSeconds(DateTime when_, int seconds) {'));
    expect(contents, contains('int addU64(int left, int right) {'));
    expect(contents, contains('int freeCount() {'));
    expect(contents, contains('Uint8List bytesEcho(Uint8List input) {'));
    expect(contents, contains('List<Uint8List> bytesChunksEcho(List<Uint8List> input) {'));
    expect(contents, contains('Uint8List? bytesMaybeEcho(Uint8List? input) {'));
    expect(contents, contains('int bytesFreeCount() {'));
    expect(contents, contains('int bytesVecFreeCount() {'));
    expect(contents, contains('int negate(int value) {'));
    expect(contents, contains('void resetBytesFreeCount() {'));
    expect(contents, contains('void resetBytesVecFreeCount() {'));
    expect(contents, contains('void resetFreeCount() {'));
    expect(contents, contains('String brokenGreet() {'));
    expect(contents, contains('String greet(String name) {'));
    expect(
      contents,
      contains('Duration multiplyDuration(Duration value, int factor) {'),
    );
    expect(contents, contains('String? maybeGreet(String? name) {'));
    expect(contents, contains('bool isEven(int value) {'));
    expect(contents, contains('double scale(double value, double factor) {'));
    expect(contents, contains('double scale32(double value, double factor) {'));
    expect(contents, contains('int subtractI64(int left, int right) {'));
    expect(contents, contains('void tick() {'));
    expect(contents, contains('int currentTick() {'));
    expect(contents, contains('late final int Function(int left, int right) _add ='));
    expect(contents, contains('late final int Function(int left, int right) _addU64 ='));
    expect(contents, contains('late final bool Function(int value) _isEven ='));
    expect(contents, contains('final class _RustBuffer extends ffi.Struct {'));
    expect(contents, contains('final class _RustBufferOpt extends ffi.Struct {'));
    expect(contents, contains('final class _RustBufferVec extends ffi.Struct {'));
    expect(contents, contains('late final void Function(_RustBuffer) _rustBytesFree ='));
    expect(contents, contains('late final void Function(_RustBufferVec) _rustBytesVecFree ='));
    expect(contents, contains('final ffi.Pointer<_RustBuffer> inputBufferPtr = calloc<_RustBuffer>();'));
    expect(contents, contains('final ffi.Pointer<_RustBufferOpt> inputOptPtr = calloc<_RustBufferOpt>();'));
    expect(contents, contains('final ffi.Pointer<_RustBufferVec> inputVecPtr = calloc<_RustBufferVec>();'));
    expect(contents, contains('ffi.Pointer<Utf8> nameNative = name.toNativeUtf8();'));
    expect(
      contents,
      contains(
        'final ffi.Pointer<Utf8> nameNative = name == null ? ffi.nullptr : name.toNativeUtf8();',
      ),
    );
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
    expect(contents, contains('when_.toUtc().microsecondsSinceEpoch'));
    expect(
      contents,
      contains(
        'return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);',
      ),
    );
    expect(contents, contains('value.inMicroseconds'));
    expect(contents, contains('return Duration(microseconds: micros);'));
    expect(contents, contains('return Uint8List.fromList(resultData.asTypedList(resultLen));'));
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
    bindings.resetFreeCount();
    bindings.resetBytesFreeCount();
    bindings.resetBytesVecFreeCount();
    expect(bindings.freeCount(), 0);
    expect(bindings.bytesFreeCount(), 0);
    expect(bindings.bytesVecFreeCount(), 0);
    expect(bindings.add(20, 22), 42);
    final baseTime = DateTime.fromMicrosecondsSinceEpoch(1000000, isUtc: true);
    expect(
      bindings.addSeconds(baseTime, 5),
      DateTime.fromMicrosecondsSinceEpoch(6000000, isUtc: true),
    );
    expect(bindings.addU64(10000000000, 25), 10000000025);
    expect(
      bindings.bytesEcho(Uint8List.fromList([1, 2, 3, 4])),
      Uint8List.fromList([1, 2, 3, 4]),
    );
    expect(bindings.bytesFreeCount(), 1);
    expect(bindings.bytesEcho(Uint8List(0)), Uint8List(0));
    expect(bindings.bytesFreeCount(), 1);
    expect(bindings.bytesMaybeEcho(Uint8List.fromList([5, 6])), Uint8List.fromList([5, 6]));
    expect(bindings.bytesFreeCount(), 2);
    expect(bindings.bytesMaybeEcho(Uint8List(0)), Uint8List(0));
    expect(bindings.bytesFreeCount(), 2);
    expect(bindings.bytesMaybeEcho(null), isNull);
    expect(bindings.bytesFreeCount(), 2);
    expect(
      bindings.bytesChunksEcho([
        Uint8List.fromList([3]),
        Uint8List(0),
        Uint8List.fromList([4, 5]),
      ]),
      [
        Uint8List.fromList([3]),
        Uint8List(0),
        Uint8List.fromList([4, 5]),
      ],
    );
    expect(bindings.bytesFreeCount(), 4);
    expect(bindings.bytesVecFreeCount(), 1);
    expect(bindings.bytesChunksEcho(<Uint8List>[]), <Uint8List>[]);
    expect(bindings.bytesVecFreeCount(), 1);
    expect(bindings.negate(7), -7);
    expect(
      bindings.subtractI64(9000000000000000000, 1000000000000000000),
      8000000000000000000,
    );
    expect(
      bindings.multiplyDuration(const Duration(milliseconds: 250), 4),
      const Duration(milliseconds: 1000),
    );
    expect(bindings.greet('dart'), 'hello, dart');
    expect(bindings.freeCount(), 1);
    expect(bindings.maybeGreet('dart'), 'maybe, dart');
    expect(bindings.freeCount(), 2);
    expect(bindings.maybeGreet(null), isNull);
    expect(bindings.freeCount(), 2);
    expect(() => bindings.brokenGreet(), throwsA(isA<StateError>()));
    expect(bindings.freeCount(), 2);
    expect(bindings.isEven(8), isTrue);
    expect(bindings.isEven(9), isFalse);
    expect(bindings.scale(2.5, 4.0), closeTo(10.0, 0.000001));
    expect(bindings.scale32(1.25, 8.0), closeTo(10.0, 0.0001));
    final before = bindings.currentTick();
    bindings.tick();
    expect(bindings.currentTick(), before + 1);

    configureDefaultBindings(libraryPath: libPath);
    resetFreeCount();
    resetBytesFreeCount();
    resetBytesVecFreeCount();
    expect(freeCount(), 0);
    expect(bytesFreeCount(), 0);
    expect(bytesVecFreeCount(), 0);
    expect(add(1, 2), 3);
    expect(
      addSeconds(baseTime, 2),
      DateTime.fromMicrosecondsSinceEpoch(3000000, isUtc: true),
    );
    expect(addU64(4000000000, 2), 4000000002);
    expect(bytesEcho(Uint8List.fromList([9, 8])), Uint8List.fromList([9, 8]));
    expect(bytesFreeCount(), 1);
    expect(bytesEcho(Uint8List(0)), Uint8List(0));
    expect(bytesFreeCount(), 1);
    expect(bytesMaybeEcho(Uint8List.fromList([7])), Uint8List.fromList([7]));
    expect(bytesFreeCount(), 2);
    expect(bytesMaybeEcho(Uint8List(0)), Uint8List(0));
    expect(bytesFreeCount(), 2);
    expect(bytesMaybeEcho(null), isNull);
    expect(bytesFreeCount(), 2);
    expect(
      bytesChunksEcho([Uint8List.fromList([1, 2]), Uint8List(0)]),
      [Uint8List.fromList([1, 2]), Uint8List(0)],
    );
    expect(bytesFreeCount(), 3);
    expect(bytesVecFreeCount(), 1);
    expect(bytesChunksEcho(<Uint8List>[]), <Uint8List>[]);
    expect(bytesVecFreeCount(), 1);
    expect(negate(5), -5);
    expect(subtractI64(5000000000, 2000000000), 3000000000);
    expect(
      multiplyDuration(const Duration(seconds: 2), 3),
      const Duration(seconds: 6),
    );
    expect(greet('ffi'), 'hello, ffi');
    expect(freeCount(), 1);
    expect(maybeGreet('ffi'), 'maybe, ffi');
    expect(freeCount(), 2);
    expect(maybeGreet(null), isNull);
    expect(freeCount(), 2);
    expect(() => brokenGreet(), throwsA(isA<StateError>()));
    expect(freeCount(), 2);
    expect(isEven(10), isTrue);
    expect(isEven(11), isFalse);
    expect(scale(1.5, 3.0), closeTo(4.5, 0.000001));
    expect(scale32(0.5, 6.0), closeTo(3.0, 0.0001));
    final globalBefore = currentTick();
    tick();
    expect(currentTick(), globalBefore + 1);
    resetDefaultBindings();
  });
}
