import 'dart:io';
import 'dart:typed_data';

import 'package:test/test.dart';
import '../generated/simple_fns.dart';

final class _TestAdder implements Adder {
  const _TestAdder(this.offset);
  final int offset;

  @override
  int add(int left, int right) => left + right + offset;

  @override
  Future<int> addAsync(int left, int right) async => left + right + offset;

  @override
  int checkedAdd(int left, int right) {
    if (right == 0) {
      throw const MathErrorExceptionDivisionByZero();
    }
    if (left == 0) {
      throw const MathErrorExceptionNegativeInput(value: -1);
    }
    return left + right + offset;
  }
}

final class _TestFormatter implements Formatter {
  const _TestFormatter();

  @override
  String format(String? prefix, Person person, Outcome outcome) {
    final String prefixPart = prefix ?? 'none';
    final String outcomeTag = switch (outcome) {
      OutcomeSuccess() => 'success',
      OutcomeFailure() => 'failure',
    };
    return '$prefixPart:${person.name}:${person.age}:$outcomeTag';
  }

  @override
  Future<String> formatAsync(String? prefix, Person person, Outcome outcome) async {
    final String prefixPart = prefix ?? 'none';
    final String outcomeTag = switch (outcome) {
      OutcomeSuccess() => 'success',
      OutcomeFailure() => 'failure',
    };
    return 'async:$prefixPart:${person.name}:${person.age}:$outcomeTag';
  }

  @override
  Future<String?> formatAsyncOptional(
    String? prefix,
    Person person,
    Outcome outcome,
  ) async {
    if (prefix == null) {
      return null;
    }
    final String outcomeTag = switch (outcome) {
      OutcomeSuccess() => 'success',
      OutcomeFailure() => 'failure',
    };
    return 'optional:$prefix:${person.name}:${person.age}:$outcomeTag';
  }

  @override
  Future<Person> formatAsyncPerson(
    String? prefix,
    Person person,
    Outcome outcome,
  ) async {
    final String prefixPart = prefix ?? 'none';
    return Person(name: '$prefixPart-${person.name}', age: person.age + 1);
  }

  @override
  Future<Outcome> formatAsyncOutcome(
    String? prefix,
    Person person,
    Outcome outcome,
  ) async {
    return switch (outcome) {
      OutcomeSuccess(message: final message) => OutcomeFailure(
        code: message.length,
        reason: prefix ?? 'none',
      ),
      OutcomeFailure(code: final code, reason: final reason) => OutcomeSuccess(
        message: '$code:$reason:${person.name}',
      ),
    };
  }
}

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
    expect(contents, contains('int applyAdder(Adder adder, int left, int right) {'));
    expect(contents, contains('Future<int> asyncApplyAdder(Adder adder, int left, int right) {'));
    expect(contents, contains('int checkedApplyAdder(Adder adder, int left, int right) {'));
    expect(contents, contains('int applyFormatter(Formatter formatter, String? prefix, Person person, Outcome outcome) {'));
    expect(contents, contains('Future<String> asyncApplyFormatter(Formatter formatter, String? prefix, Person person, Outcome outcome) {'));
    expect(contents, contains('Future<int> asyncApplyFormatterOptionalLen(Formatter formatter, String? prefix, Person person, Outcome outcome) {'));
    expect(contents, contains('Future<int> asyncApplyFormatterPersonLen(Formatter formatter, String? prefix, Person person, Outcome outcome) {'));
    expect(contents, contains('Future<int> asyncApplyFormatterOutcomeLen(Formatter formatter, String? prefix, Person person, Outcome outcome) {'));
    expect(contents, contains('Person echoPerson(Person input) {'));
    expect(contents, contains('Outcome evolveOutcome(Outcome input) {'));
    expect(contents, contains('DateTime addSeconds(DateTime when_, int seconds) {'));
    expect(contents, contains('int addU64(int left, int right) {'));
    expect(contents, contains('int freeCount() {'));
    expect(contents, contains('Uint8List bytesEcho(Uint8List input) {'));
    expect(contents, contains('List<Uint8List> bytesChunksEcho(List<Uint8List> input) {'));
    expect(contents, contains('Uint8List? bytesMaybeEcho(Uint8List? input) {'));
    expect(contents, contains('int bytesFreeCount() {'));
    expect(contents, contains('int bytesVecFreeCount() {'));
    expect(contents, contains('int checkedDivide(int left, int right) {'));
    expect(contents, contains('int negate(int value) {'));
    expect(contents, contains('void resetBytesFreeCount() {'));
    expect(contents, contains('void resetBytesVecFreeCount() {'));
    expect(contents, contains('void resetFreeCount() {'));
    expect(contents, contains('String brokenGreet() {'));
    expect(contents, contains('String greet(String name) {'));
    expect(contents, contains('Future<String> asyncGreet(String name) {'));
    expect(contents, contains('Future<int> asyncAdd(int left, int right) {'));
    expect(contents, contains('Future<void> asyncTick() {'));
    expect(contents, contains('rust_future_poll_string'));
    expect(contents, contains('rust_future_cancel_string'));
    expect(contents, contains('rust_future_complete_string'));
    expect(contents, contains('rust_future_free_string'));
    expect(contents, contains('rust_future_poll_u32'));
    expect(contents, contains('rust_future_complete_u32'));
    expect(contents, contains('rust_future_poll_void'));
    expect(contents, contains('rust_future_complete_void'));
    expect(contents, contains('final class _RustCallStatus extends ffi.Struct {'));
    expect(
      contents,
      contains('Duration multiplyDuration(Duration value, int factor) {'),
    );
    expect(contents, contains('String? maybeGreet(String? name) {'));
    expect(contents, contains('bool isEven(int value) {'));
    expect(contents, contains('Color cycleColor(Color input) {'));
    expect(contents, contains('double scale(double value, double factor) {'));
    expect(contents, contains('double scale32(double value, double factor) {'));
    expect(contents, contains('int subtractI64(int left, int right) {'));
    expect(contents, contains('void tick() {'));
    expect(contents, contains('int currentTick() {'));
    expect(contents, contains('late final int Function(int left, int right) _add ='));
    expect(contents, contains("lookupFunction<ffi.Void Function(ffi.Pointer<_AdderVTable>)"));
    expect(contents, contains("'adder_callback_init'"));
    expect(contents, contains('final class _AdderVTable extends ffi.Struct {'));
    expect(contents, contains('final class _AdderCallbackBridge {'));
    expect(contents, contains('Future<int> addAsync(int left, int right);'));
    expect(contents, contains('int checkedAdd(int left, int right);'));
    expect(contents, contains('final class _ForeignFutureDroppedCallbackStruct extends ffi.Struct {'));
    expect(contents, contains('final class _AdderAddAsyncAsyncResult extends ffi.Struct {'));
    expect(contents, contains('abstract interface class Formatter {'));
    expect(contents, contains('String format(String? prefix, Person person, Outcome outcome);'));
    expect(contents, contains('Future<String> formatAsync(String? prefix, Person person, Outcome outcome);'));
    expect(contents, contains('Future<String?> formatAsyncOptional(String? prefix, Person person, Outcome outcome);'));
    expect(contents, contains('Future<Person> formatAsyncPerson(String? prefix, Person person, Outcome outcome);'));
    expect(contents, contains('Future<Outcome> formatAsyncOutcome(String? prefix, Person person, Outcome outcome);'));
    expect(contents, contains('final class _FormatterFormatAsyncAsyncResult extends ffi.Struct {'));
    expect(contents, contains('final class _FormatterFormatAsyncOptionalAsyncResult extends ffi.Struct {'));
    expect(contents, contains('final class _FormatterFormatAsyncPersonAsyncResult extends ffi.Struct {'));
    expect(contents, contains('final class _FormatterFormatAsyncOutcomeAsyncResult extends ffi.Struct {'));
    expect(contents, contains("'formatter_callback_init'"));
    expect(contents, contains('final class _FormatterVTable extends ffi.Struct {'));
    expect(contents, contains('final class _FormatterCallbackBridge {'));
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
    expect(contents, contains('class Person {'));
    expect(contents, contains('enum Color {'));
    expect(contents, contains('sealed class Outcome {'));
    expect(contents, contains('sealed class MathErrorException implements Exception {'));
    expect(contents, contains('final class MathErrorExceptionDivisionByZero extends MathErrorException {'));
    expect(contents, contains('final class MathErrorExceptionNegativeInput extends MathErrorException {'));
    expect(contents, contains('MathErrorException _decodeMathErrorException(Object? raw) {'));
    expect(contents, contains('final class Counter {'));
    expect(contents, contains('static Counter create(int initial) {'));
    expect(contents, contains('static Counter withPerson(Person seed) {'));
    expect(contents, contains('void addValue(int amount) {'));
    expect(contents, contains('int applyAdderWith(Adder adder, int left, int right) {'));
    expect(contents, contains('Future<int> asyncApplyAdderWith(Adder adder, int left, int right) {'));
    expect(contents, contains('int checkedApplyAdderWith(Adder adder, int left, int right) {'));
    expect(
      contents,
      contains('int counterInvokeApplyAdderWith(int handle, Adder adder, int left, int right) {'),
    );
    expect(
      contents,
      contains('Future<int> counterInvokeAsyncApplyAdderWith(int handle, Adder adder, int left, int right) async {'),
    );
    expect(
      contents,
      contains('int counterInvokeCheckedApplyAdderWith(int handle, Adder adder, int left, int right) {'),
    );
    expect(contents, contains('int currentValue() {'));
    expect(contents, contains('void setLabel(String label) {'));
    expect(contents, contains('String maybeLabel(String? suffix) {'));
    expect(contents, contains('void ingestPerson(Person input) {'));
    expect(contents, contains('Outcome flipOutcome(Outcome input) {'));
    expect(contents, contains('int bytesLen(Uint8List input) {'));
    expect(contents, contains('int optionalBytesLen(Uint8List? input) {'));
    expect(contents, contains('int chunksTotalLen(List<Uint8List> input) {'));
    expect(contents, contains('String describe() {'));
    expect(contents, contains('Future<String> asyncDescribe() {'));
    expect(contents, contains('Future<int> asyncValue() {'));
    expect(contents, contains('Person snapshotPerson() {'));
    expect(contents, contains('Outcome snapshotOutcome() {'));
    expect(contents, contains('Uint8List snapshotBytes() {'));
    expect(contents, contains('int divideBy(int divisor) {'));
    expect(contents, contains('Outcome riskyOutcome(int divisor) {'));
    expect(contents, contains('void close() {'));
    expect(contents, contains('String toString() {'));
    expect(contents, contains('int get hashCode {'));
    expect(contents, contains('bool operator ==(Object other) {'));
    expect(contents, contains('return _ffi.counterInvokeUniffiTraitEq(_handle, other._handle);'));
    expect(contents, contains('return _ffi.counterInvokeUniffiTraitDisplay(_handle);'));
    expect(contents, contains('return _ffi.counterInvokeUniffiTraitHash(_handle);'));
    expect(contents, contains('final class OutcomeSuccess extends Outcome {'));
    expect(contents, contains('final class OutcomeFailure extends Outcome {'));
    expect(contents, contains('String _encodeColor(Color value) {'));
    expect(contents, contains('Color _decodeColor(String raw) {'));
    expect(contents, contains('String _encodeOutcome(Outcome value) {'));
    expect(contents, contains('Outcome _decodeOutcome(String raw) {'));
  });

  test('runtime ffi binding can call native exports', () async {
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
    expect(bindings.applyAdder(const _TestAdder(5), 20, 22), 48);
    expect(await bindings.asyncApplyAdder(const _TestAdder(3), 20, 22), 47);
    expect(bindings.checkedApplyAdder(const _TestAdder(1), 2, 3), 6);
    expect(
      () => bindings.checkedApplyAdder(const _TestAdder(1), 2, 0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    expect(
      () => bindings.checkedApplyAdder(const _TestAdder(1), 0, 3),
      throwsA(
        isA<MathErrorExceptionNegativeInput>().having(
          (e) => e.value,
          'value',
          -1,
        ),
      ),
    );
    expect(
      bindings.applyFormatter(
        const _TestFormatter(),
        'prefix',
        const Person(name: 'Ada', age: 33),
        const OutcomeSuccess(message: 'go'),
      ),
      'prefix:Ada:33:success'.length,
    );
    expect(
      await bindings.asyncApplyFormatter(
        const _TestFormatter(),
        'prefix',
        const Person(name: 'Ada', age: 33),
        const OutcomeSuccess(message: 'go'),
      ),
      'async:prefix:Ada:33:success',
    );
    expect(
      await bindings.asyncApplyFormatterOptionalLen(
        const _TestFormatter(),
        null,
        const Person(name: 'Ada', age: 33),
        const OutcomeSuccess(message: 'go'),
      ),
      0,
    );
    expect(
      await bindings.asyncApplyFormatterOptionalLen(
        const _TestFormatter(),
        'prefix',
        const Person(name: 'Ada', age: 33),
        const OutcomeSuccess(message: 'go'),
      ),
      greaterThan(0),
    );
    expect(
      await bindings.asyncApplyFormatterPersonLen(
        const _TestFormatter(),
        'prefix',
        const Person(name: 'Ada', age: 33),
        const OutcomeSuccess(message: 'go'),
      ),
      greaterThan(0),
    );
    expect(
      await bindings.asyncApplyFormatterOutcomeLen(
        const _TestFormatter(),
        'prefix',
        const Person(name: 'Ada', age: 33),
        const OutcomeSuccess(message: 'go'),
      ),
      greaterThan(0),
    );
    final echoed = bindings.echoPerson(const Person(name: 'Ada', age: 33));
    expect(echoed.name, 'Ada');
    expect(echoed.age, 33);
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
    expect(await bindings.asyncGreet('dart'), 'async, dart');
    expect(await bindings.asyncAdd(20, 22), 42);
    final asyncTickBefore = bindings.currentTick();
    await bindings.asyncTick();
    expect(bindings.currentTick(), asyncTickBefore + 1);
    expect(bindings.freeCount(), 7);
    expect(bindings.checkedDivide(12, 3), 4);
    expect(
      () => bindings.checkedDivide(10, 0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    expect(
      () => bindings.checkedDivide(-8, 2),
      throwsA(
        isA<MathErrorExceptionNegativeInput>().having(
          (e) => e.value,
          'value',
          -8,
        ),
      ),
    );
    expect(bindings.freeCount(), 10);
    final counter = bindings.counterCreateNew(10);
    expect(counter.currentValue(), 10);
    counter.addValue(5);
    expect(counter.currentValue(), 15);
    expect(counter.applyAdderWith(const _TestAdder(1), 2, 3), 21);
    expect(await counter.asyncApplyAdderWith(const _TestAdder(1), 2, 3), 21);
    expect(counter.checkedApplyAdderWith(const _TestAdder(1), 2, 3), 21);
    expect(
      () => counter.checkedApplyAdderWith(const _TestAdder(1), 2, 0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    expect(
      () => counter.checkedApplyAdderWith(const _TestAdder(1), 0, 3),
      throwsA(
        isA<MathErrorExceptionNegativeInput>().having(
          (e) => e.value,
          'value',
          -1,
        ),
      ),
    );
    expect(counter.divideBy(3), 5);
    expect(counter.currentValue(), 5);
    counter.setLabel('alpha');
    expect(counter.describe(), 'counter:5:alpha');
    expect(counter.toString(), 'counter:5:alpha');
    expect(counter.hashCode, isNot(0));
    final equalCounter = bindings.counterCreateNew(5);
    equalCounter.setLabel('alpha');
    expect(counter == equalCounter, isTrue);
    equalCounter.setLabel('beta');
    expect(counter == equalCounter, isFalse);
    equalCounter.close();
    expect(await counter.asyncDescribe(), 'async:counter:5:alpha');
    expect(await counter.asyncValue(), 5);
    expect(counter.maybeLabel('tail'), 'counter:5:tail');
    expect(counter.maybeLabel(null), 'counter:5:none');
    counter.ingestPerson(const Person(name: 'Eve', age: 9));
    expect(counter.currentValue(), 9);
    final flipped = counter.flipOutcome(const OutcomeSuccess(message: 'ok'));
    expect(flipped, isA<OutcomeFailure>());
    final flippedFailure = flipped as OutcomeFailure;
    expect(flippedFailure.code, 2);
    expect(flippedFailure.reason, 'ok');
    expect(counter.bytesLen(Uint8List.fromList([1, 2, 3])), 3);
    expect(counter.optionalBytesLen(Uint8List.fromList([9, 8])), 2);
    expect(counter.optionalBytesLen(null), 0);
    expect(
      counter.chunksTotalLen([Uint8List.fromList([1]), Uint8List(0), Uint8List.fromList([2, 3])]),
      3,
    );
    final snapPerson = counter.snapshotPerson();
    expect(snapPerson.name, 'counter');
    expect(snapPerson.age, 9);
    final snapOutcome = counter.snapshotOutcome();
    expect(snapOutcome, isA<OutcomeFailure>());
    final snapOutcomeFailure = snapOutcome as OutcomeFailure;
    expect(snapOutcomeFailure.code, 9);
    expect(snapOutcomeFailure.reason, 'odd');
    final bytesBeforeSnapshot = bindings.bytesFreeCount();
    expect(counter.snapshotBytes(), Uint8List.fromList('bytes:9'.codeUnits));
    expect(bindings.bytesFreeCount(), bytesBeforeSnapshot + 1);
    final risky = counter.riskyOutcome(5);
    expect(risky, isA<OutcomeSuccess>());
    final riskySuccess = risky as OutcomeSuccess;
    expect(riskySuccess.message, 'q:1');
    expect(
      () => counter.riskyOutcome(0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    expect(() => counter.divideBy(0), throwsA(isA<MathErrorExceptionDivisionByZero>()));
    expect(
      () => counter.divideBy(-2),
      throwsA(
        isA<MathErrorExceptionNegativeInput>().having((e) => e.value, 'value', -2),
      ),
    );
    counter.close();
    expect(() => counter.currentValue(), throwsA(isA<StateError>()));
    final counterFromPerson = bindings.counterCreateWithPerson(
      const Person(name: 'Neo', age: 4),
    );
    expect(counterFromPerson.currentValue(), 4);
    counterFromPerson.close();
    final freeAfterCounter = bindings.freeCount();
    expect(freeAfterCounter, greaterThanOrEqualTo(9));
    expect(bindings.maybeGreet('dart'), 'maybe, dart');
    expect(bindings.freeCount(), freeAfterCounter + 1);
    expect(bindings.maybeGreet(null), isNull);
    expect(bindings.freeCount(), freeAfterCounter + 1);
    expect(() => bindings.brokenGreet(), throwsA(isA<StateError>()));
    expect(bindings.freeCount(), freeAfterCounter + 1);
    expect(bindings.isEven(8), isTrue);
    expect(bindings.cycleColor(Color.red), Color.green);
    expect(bindings.cycleColor(Color.green), Color.blue);
    expect(bindings.cycleColor(Color.blue), Color.red);
    final freeBeforeOutcome = bindings.freeCount();
    final evolved1 = bindings.evolveOutcome(const OutcomeSuccess(message: 'ok'));
    expect(evolved1, isA<OutcomeFailure>());
    final evolved1Failure = evolved1 as OutcomeFailure;
    expect(evolved1Failure.code, 2);
    expect(evolved1Failure.reason, 'ok');
    final evolved2 = bindings.evolveOutcome(
      const OutcomeFailure(code: 7, reason: 'bad'),
    );
    expect(evolved2, isA<OutcomeSuccess>());
    final evolved2Success = evolved2 as OutcomeSuccess;
    expect(evolved2Success.message, '7:bad');
    expect(bindings.freeCount(), freeBeforeOutcome + 2);
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
    expect(applyAdder(const _TestAdder(2), 3, 4), 10);
    expect(await asyncApplyAdder(const _TestAdder(1), 3, 4), 10);
    expect(checkedApplyAdder(const _TestAdder(1), 3, 4), 8);
    expect(
      () => checkedApplyAdder(const _TestAdder(1), 3, 0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    expect(
      () => checkedApplyAdder(const _TestAdder(1), 0, 4),
      throwsA(
        isA<MathErrorExceptionNegativeInput>().having(
          (e) => e.value,
          'value',
          -1,
        ),
      ),
    );
    expect(
      applyFormatter(
        const _TestFormatter(),
        null,
        const Person(name: 'Lin', age: 10),
        const OutcomeFailure(code: 7, reason: 'bad'),
      ),
      'none:Lin:10:failure'.length,
    );
    expect(
      await asyncApplyFormatter(
        const _TestFormatter(),
        null,
        const Person(name: 'Lin', age: 10),
        const OutcomeFailure(code: 7, reason: 'bad'),
      ),
      'async:none:Lin:10:failure',
    );
    expect(
      await asyncApplyFormatterOptionalLen(
        const _TestFormatter(),
        null,
        const Person(name: 'Lin', age: 10),
        const OutcomeFailure(code: 7, reason: 'bad'),
      ),
      0,
    );
    expect(
      await asyncApplyFormatterPersonLen(
        const _TestFormatter(),
        'px',
        const Person(name: 'Lin', age: 10),
        const OutcomeFailure(code: 7, reason: 'bad'),
      ),
      greaterThan(0),
    );
    expect(
      await asyncApplyFormatterOutcomeLen(
        const _TestFormatter(),
        'px',
        const Person(name: 'Lin', age: 10),
        const OutcomeFailure(code: 7, reason: 'bad'),
      ),
      greaterThan(0),
    );
    final echoedTop = echoPerson(const Person(name: 'Lin', age: 10));
    expect(echoedTop.name, 'Lin');
    expect(echoedTop.age, 10);
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
    expect(await asyncGreet('ffi'), 'async, ffi');
    expect(await asyncAdd(4, 8), 12);
    final asyncGlobalBefore = currentTick();
    await asyncTick();
    expect(currentTick(), asyncGlobalBefore + 1);
    expect(freeCount(), 7);
    expect(checkedDivide(20, 4), 5);
    expect(
      () => checkedDivide(7, 0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    expect(
      () => checkedDivide(4, -2),
      throwsA(
        isA<MathErrorExceptionNegativeInput>().having(
          (e) => e.value,
          'value',
          -2,
        ),
      ),
    );
    expect(freeCount(), 10);
    expect(maybeGreet('ffi'), 'maybe, ffi');
    expect(freeCount(), 11);
    expect(maybeGreet(null), isNull);
    expect(freeCount(), 11);
    expect(() => brokenGreet(), throwsA(isA<StateError>()));
    expect(freeCount(), 11);
    expect(isEven(10), isTrue);
    expect(cycleColor(Color.red), Color.green);
    final freeBeforeTopOutcome = freeCount();
    final topEvolved = evolveOutcome(const OutcomeSuccess(message: 'go'));
    expect(topEvolved, isA<OutcomeFailure>());
    final topFailure = topEvolved as OutcomeFailure;
    expect(topFailure.code, 2);
    expect(topFailure.reason, 'go');
    expect(freeCount(), freeBeforeTopOutcome + 1);
    expect(isEven(11), isFalse);
    expect(scale(1.5, 3.0), closeTo(4.5, 0.000001));
    expect(scale32(0.5, 6.0), closeTo(3.0, 0.0001));
    final topCounter = Counter.create(3);
    expect(topCounter.applyAdderWith(const _TestAdder(2), 2, 3), 10);
    expect(topCounter.toString(), 'counter:3');
    expect(topCounter.hashCode, isNot(0));
    final topCounterSame = Counter.create(3);
    expect(topCounter == topCounterSame, isTrue);
    topCounterSame.close();
    expect(await topCounter.asyncApplyAdderWith(const _TestAdder(2), 2, 3), 10);
    expect(topCounter.checkedApplyAdderWith(const _TestAdder(2), 2, 3), 10);
    expect(
      () => topCounter.checkedApplyAdderWith(const _TestAdder(2), 2, 0),
      throwsA(isA<MathErrorExceptionDivisionByZero>()),
    );
    topCounter.close();
    final globalBefore = currentTick();
    tick();
    expect(currentTick(), globalBefore + 1);
    resetDefaultBindings();
  });
}
