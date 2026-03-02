import 'dart:io';
import 'dart:typed_data';

import 'package:test/test.dart';
import '../generated/coverall_demo.dart';

void main() {
  late String libPath;

  setUp(() {
    final path = Platform.environment['UBDG_COVERALL_DEMO_LIB'];
    expect(
      path,
      isNotNull,
      reason:
          'UBDG_COVERALL_DEMO_LIB must point to the compiled coverall-demo fixture library',
    );
    libPath = path!;
    configureDefaultBindings(libraryPath: libPath);
  });

  tearDown(() {
    resetDefaultBindings();
  });

  // Not tested: makeRustGetters, ancestorNames
  // makeRustGetters returns a callback interface type (unsupported).
  // ancestorNames takes a NodeTrait (recursive callback, unsupported).

  test('generated bindings file exists', () {
    final generated = File('generated/coverall_demo.dart');
    expect(generated.existsSync(), isTrue);
  });

  group('top-level functions', () {
    test('createSomeDict returns populated dict', () {
      final dict = createSomeDict();
      expect(dict.text, 'hello');
      expect(dict.maybeCount, 42);
      expect(dict.flag, isTrue);
      expect(dict.color, Color.red);
      expect(dict.tags, ['a', 'b']);
      expect(dict.counts['x'], 1);
      expect(dict.counts['y'], 2);
      expect(dict.maybeText, 'present');
      expect(dict.maybePatch, isNull);
      expect(dict.maybeU8, 1);
      expect(dict.maybeU16, 2);
      expect(dict.maybeU64, 3);
      expect(dict.maybeI8, -1);
      expect(dict.maybeI64, -2);
      expect(dict.maybeF32, closeTo(1.5, 0.01));
      expect(dict.maybeF64, closeTo(2.5, 0.01));
      expect(dict.maybeBool, isTrue);
      expect(dict.maybeBytes, isNull);
      expect(dict.someBytes, isEmpty);
      expect(dict.coveralls, isNull);
    });

    test('createNoneDict returns dict with nulls', () {
      final dict = createNoneDict();
      expect(dict.text, 'none');
      expect(dict.maybeCount, 0);
      expect(dict.flag, isFalse);
      expect(dict.color, Color.blue);
      expect(dict.tags, isEmpty);
      expect(dict.counts, isEmpty);
      expect(dict.maybeText, isNull);
      expect(dict.maybePatch, isNull);
      expect(dict.maybeU8, isNull);
      expect(dict.maybeU16, isNull);
      expect(dict.maybeU64, isNull);
      expect(dict.maybeI8, isNull);
      expect(dict.maybeI64, isNull);
      expect(dict.maybeF32, isNull);
      expect(dict.maybeF64, isNull);
      expect(dict.maybeBool, isNull);
      expect(dict.maybeBytes, isNull);
      expect(dict.someBytes, isEmpty);
      expect(dict.coveralls, isNull);
    });

    test('getMaybeSimpleDict Nah variant', () {
      final result = getMaybeSimpleDict(0);
      expect(result, isA<MaybeSimpleDictNah>());
    });

    test('getMaybeSimpleDict Yeah variant', () {
      final result = getMaybeSimpleDict(1);
      expect(result, isA<MaybeSimpleDictYeah>());
      final yeah = result as MaybeSimpleDictYeah;
      expect(yeah.value.text, 'from_index');
      expect(yeah.value.maybeCount, 1);
      expect(yeah.value.flag, isTrue);
      expect(yeah.value.color, Color.green);
    });

    test('println round-trips a string', () {
      expect(println('hello world'), 'hello world');
    });

    test('println throws ComplexError OsError', () {
      expect(
        () => println('os_error'),
        throwsA(isA<ComplexErrorExceptionOsError>()
            .having((e) => e.code, 'code', 42)),
      );
    });

    test('println throws ComplexError PermissionDenied', () {
      expect(
        () => println('permission'),
        throwsA(isA<ComplexErrorExceptionPermissionDenied>()
            .having((e) => e.reason, 'reason', 'nope')),
      );
    });

    test('println throws ComplexError UnknownError', () {
      expect(
        () => println('unknown'),
        throwsA(isA<ComplexErrorExceptionUnknownError>()),
      );
    });

    test('divideByText success', () {
      expect(divideByText(10.0, '2'), closeTo(5.0, 0.0001));
    });

    test('divideByText division by zero', () {
      expect(
        () => divideByText(10.0, '0'),
        throwsA(isA<ComplexErrorExceptionOsError>()),
      );
    });

    test('divideByText non-numeric divisor', () {
      expect(
        () => divideByText(10.0, 'abc'),
        throwsA(isA<ComplexErrorExceptionPermissionDenied>()
            .having((e) => e.reason, 'reason', 'not a number')),
      );
    });

    test('reverseBytes', () {
      expect(
        reverseBytes(Uint8List.fromList([1, 2, 3, 4])),
        Uint8List.fromList([4, 3, 2, 1]),
      );
    });

    test('reverseBytes empty', () {
      expect(reverseBytes(Uint8List(0)), Uint8List(0));
    });

    test('getNumAlive tracks instances', () {
      final before = getNumAlive();
      final c = Coveralls.create('test');
      expect(getNumAlive(), before + 1);
      c.close();
      expect(getNumAlive(), before);
    });

    test('getMaybeCount returns value when true', () {
      final result = getMaybeCount(true);
      expect(result, 42);
    });

    test('getMaybeCount returns null when false', () {
      final result = getMaybeCount(false);
      expect(result, isNull);
    });

    test('getMaybeFlag returns value when true', () {
      final result = getMaybeFlag(true);
      expect(result, isTrue);
    });

    test('getMaybeFlag returns null when false', () {
      final result = getMaybeFlag(false);
      expect(result, isNull);
    });

    test('getMaybeDict returns dict when true', () {
      final result = getMaybeDict(true);
      expect(result, isNotNull);
      expect(result!.text, 'hello');
      expect(result.maybeCount, 42);
    });

    test('getMaybeDict returns null when false', () {
      final result = getMaybeDict(false);
      expect(result, isNull);
    });

    test('describeMaybeDict with value', () {
      final dict = createSomeDict();
      final result = describeMaybeDict(dict);
      expect(result, startsWith('dict:'));
    });

    test('describeMaybeDict with null', () {
      final result = describeMaybeDict(null);
      expect(result, 'null');
    });

    test('getMaybeColor returns color when true', () {
      final result = getMaybeColor(true);
      expect(result, Color.red);
    });

    test('getMaybeColor returns null when false', () {
      final result = getMaybeColor(false);
      expect(result, isNull);
    });

    test('describeMaybeColor with value', () {
      final result = describeMaybeColor(Color.green);
      expect(result, startsWith('color:'));
    });

    test('describeMaybeColor with null', () {
      final result = describeMaybeColor(null);
      expect(result, 'null');
    });

    test('getIntMap round-trip', () {
      final result = getIntMap(31, 42);
      expect(result[31], 42);
    });

    test('throwFlatError throws CoverallFlatError', () {
      expect(
        () => throwFlatError(),
        throwsA(isA<CoverallFlatErrorExceptionTooManyVariants>()),
      );
    });

    test('validateHtml empty source succeeds', () {
      validateHtml('');
    });

    test('validateHtml non-empty throws HTMLError', () {
      expect(
        () => validateHtml('<invalid>'),
        throwsA(isA<HTMLErrorExceptionInvalidHTML>()),
      );
    });

    test('outputReturnOnlyDict returns dict with error enum', () {
      final result = outputReturnOnlyDict();
      expect(result.e, isA<CoverallFlatError>());
    });

    test('outputReturnOnlyEnum returns One variant', () {
      final result = outputReturnOnlyEnum();
      expect(result, isA<ReturnOnlyEnumOne>());
      final one = result as ReturnOnlyEnumOne;
      expect(one.e, isA<CoverallFlatError>());
    });
  });

  group('Patch object', () {
    test('create and getColor', () {
      final patch = Patch.create(Color.green);
      expect(patch.getColor(), Color.green);
      patch.close();
    });

    test('createPatch top-level function', () {
      final patch = createPatch(Color.blue);
      expect(patch.getColor(), Color.blue);
      patch.close();
    });
  });

  group('FalliblePatch object', () {
    test('successful creation', () {
      final patch = FalliblePatch.create(Color.red, false);
      expect(patch.getColor(), Color.red);
      patch.close();
    });

    test('fallible creation throws', () {
      expect(
        () => FalliblePatch.create(Color.red, true),
        throwsA(isA<CoverallErrorExceptionTooManyHoles>()),
      );
    });

    test('secondary constructor success', () {
      final patch = FalliblePatch.secondary(Color.blue, false);
      expect(patch.getColor(), Color.blue);
      patch.close();
    });

    test('secondary constructor throws', () {
      expect(
        () => FalliblePatch.secondary(Color.blue, true),
        throwsA(isA<CoverallErrorExceptionTooManyHoles>()),
      );
    });
  });

  group('Coveralls object', () {
    test('constructor and getName', () {
      final c = Coveralls.create('Ada');
      expect(c.getName(), 'Ada');
      c.close();
    });

    test('fallible constructor success', () {
      final c = Coveralls.fallibleNew('Bob', false);
      expect(c.getName(), 'Bob');
      c.close();
    });

    test('fallible constructor failure', () {
      expect(
        () => Coveralls.fallibleNew('Fail', true),
        throwsA(isA<CoverallErrorExceptionTooManyHoles>()),
      );
    });

    test('setName and getName', () {
      final c = Coveralls.create('original');
      expect(c.getName(), 'original');
      c.setName('updated');
      expect(c.getName(), 'updated');
      c.close();
    });

    test('getStatus pass-through', () {
      final c = Coveralls.create('test');
      expect(c.getStatus('active'), 'active');
      expect(c.getStatus('idle'), 'idle');
      c.close();
    });

    test('getDict2 string-keyed map', () {
      final c = Coveralls.create('test');
      final result = c.getDict2('hello', 42);
      expect(result['hello'], 42);
      c.close();
    });

    test('getDict3 non-string-keyed map', () {
      final c = Coveralls.create('test');
      final result = c.getDict3(7, 99);
      expect(result[7], 99);
      c.close();
    });

    test('addPatch does not throw', () {
      final c = Coveralls.create('test');
      final p = Patch.create(Color.red);
      c.addPatch(p);
      p.close();
      c.close();
    });

    test('addRepair and getRepairs', () {
      final c = Coveralls.create('test');
      final p = Patch.create(Color.green);
      final now = DateTime.now().toUtc();
      c.addRepair(Repair(when_: now, patch: p));
      final repairs = c.getRepairs();
      expect(repairs, hasLength(1));
      p.close();
      c.close();
    });

    test('setAndGetEmptyStruct', () {
      final c = Coveralls.create('test');
      final empty = EmptyStruct();
      final result = c.setAndGetEmptyStruct(empty);
      expect(result, isA<EmptyStruct>());
      c.close();
    });

    test('strongCount', () {
      final c = Coveralls.create('strong');
      expect(c.strongCount(), 1);
      c.close();
    });

    test('cloneMe', () {
      final c = Coveralls.create('cloneable');
      final clone = c.cloneMe();
      expect(clone.getName(), 'cloneable');
      c.close();
      clone.close();
    });

    test('maybeThrow does not throw', () {
      final c = Coveralls.create('thrower');
      expect(c.maybeThrow(false), isTrue);
      c.close();
    });

    test('maybeThrow throws CoverallError', () {
      final c = Coveralls.create('thrower');
      expect(
        () => c.maybeThrow(true),
        throwsA(isA<CoverallErrorExceptionTooManyHoles>()),
      );
      c.close();
    });

    test('maybeThrowInto does not throw', () {
      final c = Coveralls.create('thrower');
      expect(c.maybeThrowInto(false), isTrue);
      c.close();
    });

    test('maybeThrowInto throws CoverallError', () {
      final c = Coveralls.create('thrower');
      expect(
        () => c.maybeThrowInto(true),
        throwsA(isA<CoverallErrorExceptionTooManyHoles>()),
      );
      c.close();
    });

    test('maybeThrowComplex success', () {
      final c = Coveralls.create('complex');
      expect(c.maybeThrowComplex(0), isTrue);
      c.close();
    });

    test('maybeThrowComplex OsError', () {
      final c = Coveralls.create('complex');
      expect(
        () => c.maybeThrowComplex(1),
        throwsA(isA<ComplexErrorExceptionOsError>()
            .having((e) => e.code, 'code', 10)
            .having((e) => e.extendedCode, 'extendedCode', 20)),
      );
      c.close();
    });

    test('maybeThrowComplex PermissionDenied', () {
      final c = Coveralls.create('complex');
      expect(
        () => c.maybeThrowComplex(2),
        throwsA(isA<ComplexErrorExceptionPermissionDenied>()
            .having((e) => e.reason, 'reason', 'access denied')),
      );
      c.close();
    });

    test('maybeThrowComplex UnknownError', () {
      final c = Coveralls.create('complex');
      expect(
        () => c.maybeThrowComplex(3),
        throwsA(isA<ComplexErrorExceptionUnknownError>()),
      );
      c.close();
    });

    test('reverseBytes on object', () {
      final c = Coveralls.create('bytes');
      expect(
        c.reverseBytes(Uint8List.fromList([5, 6, 7])),
        Uint8List.fromList([7, 6, 5]),
      );
      c.close();
    });

    test('getMetadata', () {
      final c = Coveralls.create('meta');
      final metadata = c.getMetadata();
      expect(metadata['name'], 'meta');
      expect(metadata.containsKey('version'), isTrue);
      expect(metadata['version'], isNull);
      c.close();
    });

    test('takeOther and getOther', () {
      final c1 = Coveralls.create('parent');
      final c2 = Coveralls.create('child');
      c1.takeOther(c2);
      final other = c1.getOther();
      expect(other, isNotNull);
      other!.close();
      c1.takeOther(null);
      expect(c1.getOther(), isNull);
      c1.close();
      c2.close();
    });

    test('getTags returns sequence with nulls', () {
      final c = Coveralls.create('tagged');
      final tags = c.getTags();
      expect(tags, hasLength(3));
      expect(tags[0], 'tagged');
      expect(tags[1], isNull);
      expect(tags[2], 'tag');
      c.close();
    });

    test('close prevents further use', () {
      final c = Coveralls.create('closable');
      c.close();
      expect(() => c.getName(), throwsA(isA<StateError>()));
    });
  });

  group('ThreadsafeCounter', () {
    test('create and increment', () {
      final counter = ThreadsafeCounter.create();
      expect(counter.getCount(), 0);
      counter.increment();
      expect(counter.getCount(), 1);
      counter.increment();
      counter.increment();
      expect(counter.getCount(), 3);
      counter.close();
    });
  });

  group('IFirst / ISecond forward references', () {
    test('IFirst compare with null', () {
      final first = IFirst.create();
      expect(first.compare(), isFalse);
      first.close();
    });

    test('IFirst compare with ISecond', () {
      final first = IFirst.create();
      final second = ISecond.create();
      expect(first.compare(other: second), isTrue);
      first.close();
      second.close();
    });

    test('ISecond compare with null', () {
      final second = ISecond.create();
      expect(second.compare(null), isFalse);
      second.close();
    });

    test('ISecond compare with IFirst', () {
      final first = IFirst.create();
      final second = ISecond.create();
      expect(second.compare(first), isTrue);
      first.close();
      second.close();
    });
  });

  group('EmptyStruct', () {
    test('construction', () {
      final empty = EmptyStruct();
      expect(empty, isA<EmptyStruct>());
    });

    test('toJson/fromJson round-trip', () {
      final empty = EmptyStruct();
      final json = empty.toJson();
      final restored = EmptyStruct.fromJson(json);
      expect(restored, isA<EmptyStruct>());
    });
  });

  group('DictWithDefaults', () {
    test('default values', () {
      final dict = DictWithDefaults();
      expect(dict.name, 'default-value');
      expect(dict.category, isNull);
      expect(dict.integer, 31);
      expect(dict.itemList, isEmpty);
      expect(dict.itemMap, isEmpty);
    });

    test('custom values override defaults', () {
      final dict = DictWithDefaults(name: 'custom', integer: 99);
      expect(dict.name, 'custom');
      expect(dict.integer, 99);
    });
  });

  group('MaybeObject enum', () {
    test('Obj variant with Patch', () {
      final patch = Patch.create(Color.red);
      final obj = MaybeObjectObj(p: patch);
      expect(obj, isA<MaybeObjectObj>());
      expect(obj.p.getColor(), Color.red);
      patch.close();
    });

    test('Nah variant', () {
      final nah = MaybeObjectNah();
      expect(nah, isA<MaybeObjectNah>());
    });
  });

  group('type codecs', () {
    test('Color encode/decode round-trip', () {
      for (final color in Color.values) {
        expect(ColorFfiCodec.decode(ColorFfiCodec.encode(color)), color);
      }
    });

    test('MaybeSimpleDict encode/decode round-trip Nah', () {
      final nah = MaybeSimpleDictNah();
      final decoded =
          MaybeSimpleDictFfiCodec.decode(MaybeSimpleDictFfiCodec.encode(nah));
      expect(decoded, isA<MaybeSimpleDictNah>());
    });

    test('ComplexError encode/decode round-trip', () {
      final osErr =
          ComplexErrorOsError(code: 1, extendedCode: 2);
      final decoded =
          ComplexErrorFfiCodec.decode(ComplexErrorFfiCodec.encode(osErr));
      expect(decoded, isA<ComplexErrorOsError>());
      expect((decoded as ComplexErrorOsError).code, 1);
      expect(decoded.extendedCode, 2);

      final permErr = ComplexErrorPermissionDenied(reason: 'denied');
      final decodedPerm =
          ComplexErrorFfiCodec.decode(ComplexErrorFfiCodec.encode(permErr));
      expect(decodedPerm, isA<ComplexErrorPermissionDenied>());
      expect((decodedPerm as ComplexErrorPermissionDenied).reason, 'denied');

      final unknownErr = ComplexErrorUnknownError();
      final decodedUnknown =
          ComplexErrorFfiCodec.decode(ComplexErrorFfiCodec.encode(unknownErr));
      expect(decodedUnknown, isA<ComplexErrorUnknownError>());
    });

    test('CoverallError encode/decode round-trip', () {
      final err = CoverallErrorTooManyHoles();
      final decoded =
          CoverallErrorFfiCodec.decode(CoverallErrorFfiCodec.encode(err));
      expect(decoded, isA<CoverallErrorTooManyHoles>());
    });

    test('CoverallFlatError encode/decode round-trip', () {
      final err = CoverallFlatErrorTooManyVariants();
      final decoded =
          CoverallFlatErrorFfiCodec.decode(CoverallFlatErrorFfiCodec.encode(err));
      expect(decoded, isA<CoverallFlatErrorTooManyVariants>());
    });

    test('HTMLError encode/decode round-trip', () {
      final err = HTMLErrorInvalidHTML();
      final decoded =
          HTMLErrorFfiCodec.decode(HTMLErrorFfiCodec.encode(err));
      expect(decoded, isA<HTMLErrorInvalidHTML>());
    });
  });

  group('SimpleDict model', () {
    test('toJson/fromJson round-trip', () {
      final dict = SimpleDict(
        text: 'test',
        maybeCount: 7,
        flag: true,
        color: Color.green,
        tags: ['x'],
        counts: {'k': 3},
        maybeText: 'opt',
        maybePatch: null,
        maybeU8: 10,
        maybeU16: 20,
        maybeU64: 30,
        maybeI8: -5,
        maybeI64: -10,
        maybeF32: 1.1,
        maybeF64: 2.2,
        maybeBool: false,
        maybeBytes: null,
        someBytes: Uint8List(0),
        coveralls: null,
      );
      final json = dict.toJson();
      final restored = SimpleDict.fromJson(json);
      expect(restored.text, 'test');
      expect(restored.maybeCount, 7);
      expect(restored.flag, isTrue);
      expect(restored.color, Color.green);
      expect(restored.tags, ['x']);
      expect(restored.counts['k'], 3);
      expect(restored.maybeText, 'opt');
      expect(restored.maybePatch, isNull);
      expect(restored.maybeU8, 10);
      expect(restored.maybeU16, 20);
      expect(restored.maybeU64, 30);
      expect(restored.maybeI8, -5);
      expect(restored.maybeI64, -10);
    });

    test('copyWith', () {
      final dict = SimpleDict(
        text: 'orig',
        maybeCount: 1,
        flag: false,
        color: Color.red,
        tags: [],
        counts: {},
        maybeText: null,
        maybePatch: null,
        maybeU8: null,
        maybeU16: null,
        maybeU64: null,
        maybeI8: null,
        maybeI64: null,
        maybeF32: null,
        maybeF64: null,
        maybeBool: null,
        maybeBytes: null,
        someBytes: Uint8List(0),
        coveralls: null,
      );
      final updated = dict.copyWith(text: 'new', flag: true);
      expect(updated.text, 'new');
      expect(updated.flag, isTrue);
      expect(updated.maybeCount, 1);
    });
  });

  group('ReturnOnlyDict', () {
    test('construction with error enum field', () {
      final dict = ReturnOnlyDict(e: CoverallFlatErrorTooManyVariants());
      expect(dict.e, isA<CoverallFlatErrorTooManyVariants>());
    });
  });

  group('Getters callback', () {
    test('testGetters exercises callback interface', () {
      final before = getNumAlive();
      final getters = _TestGetters();
      testGetters(getters);
      expect(getters.getBoolCalled, isTrue);
      expect(getters.getStringCalled, isTrue);
      expect(getters.getNothingCalled, isTrue);
      expect(getters.roundTripObjectCalled, isTrue);
      // The native side creates and frees its Coveralls handles, so
      // the live count should return to baseline.
      expect(getNumAlive(), before);
    });
  });
}

/// Test implementation of the Getters callback interface.
class _TestGetters implements Getters {
  bool getBoolCalled = false;
  bool getStringCalled = false;
  bool getNothingCalled = false;
  bool roundTripObjectCalled = false;

  @override
  bool getBool(bool v, bool arg2) {
    getBoolCalled = true;
    return v && arg2;
  }

  @override
  String getString(String v, bool arg2) {
    getStringCalled = true;
    if (arg2) {
      throw CoverallErrorExceptionTooManyHoles();
    }
    return v;
  }

  @override
  String? getOption(String v, bool arg2) {
    if (arg2) {
      throw ComplexErrorExceptionOsError(code: 1, extendedCode: 2);
    }
    return v.isEmpty ? null : v;
  }

  @override
  List<int> getList(List<int> v, bool arg2) {
    return v;
  }

  @override
  void getNothing(String v) {
    getNothingCalled = true;
  }

  @override
  Coveralls roundTripObject(Coveralls coveralls) {
    roundTripObjectCalled = true;
    return coveralls;
  }
}
