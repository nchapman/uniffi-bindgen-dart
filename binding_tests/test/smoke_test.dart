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
    expect(contents, contains('late final int Function(int left, int right) _add ='));
    expect(contents, contains('void configureDefaultBindings('));
    expect(contents, contains('return _bindings().add(left, right);'));
  });

  test('runtime ffi binding can call native add', () {
    final libPath = Platform.environment['UBDG_SIMPLE_FNS_LIB'];
    expect(
      libPath,
      isNotNull,
      reason:
          'UBDG_SIMPLE_FNS_LIB must point to the compiled simple-fns fixture library',
    );

    final bindings = SimpleFnsBindings(libraryPath: libPath);
    expect(bindings.add(20, 22), 42);

    configureDefaultBindings(libraryPath: libPath);
    expect(add(1, 2), 3);
    resetDefaultBindings();
  });
}
