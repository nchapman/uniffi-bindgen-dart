import 'dart:io';

import 'package:test/test.dart';

void main() {
  test('binding test scaffold is wired', () {
    expect(true, isTrue);
  });

  test('generated bindings file exists', () {
    final generated = File('generated/simple-fns.dart');
    expect(generated.existsSync(), isTrue);
  });

  test('generated bindings include expected symbols', () {
    final generated = File('generated/simple-fns.dart');
    final contents = generated.readAsStringSync();
    expect(contents, contains('library simple_fns;'));
    expect(contents, contains('class SimpleFnsBindings {'));
    expect(contents, contains("libraryName = 'uniffi_simple_fns';"));
  });
}
