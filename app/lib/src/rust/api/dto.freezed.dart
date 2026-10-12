// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'dto.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$EntropyOptions {

 Object get field0;



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is EntropyOptions&&const DeepCollectionEquality().equals(other.field0, field0));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(field0));

@override
String toString() {
  return 'EntropyOptions(field0: $field0)';
}


}

/// @nodoc
class $EntropyOptionsCopyWith<$Res>  {
$EntropyOptionsCopyWith(EntropyOptions _, $Res Function(EntropyOptions) __);
}


/// Adds pattern-matching-related methods to [EntropyOptions].
extension EntropyOptionsPatterns on EntropyOptions {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( EntropyOptions_Password value)?  password,TResult Function( EntropyOptions_Passphrase value)?  passphrase,required TResult orElse(),}){
final _that = this;
switch (_that) {
case EntropyOptions_Password() when password != null:
return password(_that);case EntropyOptions_Passphrase() when passphrase != null:
return passphrase(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( EntropyOptions_Password value)  password,required TResult Function( EntropyOptions_Passphrase value)  passphrase,}){
final _that = this;
switch (_that) {
case EntropyOptions_Password():
return password(_that);case EntropyOptions_Passphrase():
return passphrase(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( EntropyOptions_Password value)?  password,TResult? Function( EntropyOptions_Passphrase value)?  passphrase,}){
final _that = this;
switch (_that) {
case EntropyOptions_Password() when password != null:
return password(_that);case EntropyOptions_Passphrase() when passphrase != null:
return passphrase(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( PasswordOptions field0)?  password,TResult Function( PassphraseOptions field0)?  passphrase,required TResult orElse(),}) {final _that = this;
switch (_that) {
case EntropyOptions_Password() when password != null:
return password(_that.field0);case EntropyOptions_Passphrase() when passphrase != null:
return passphrase(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( PasswordOptions field0)  password,required TResult Function( PassphraseOptions field0)  passphrase,}) {final _that = this;
switch (_that) {
case EntropyOptions_Password():
return password(_that.field0);case EntropyOptions_Passphrase():
return passphrase(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( PasswordOptions field0)?  password,TResult? Function( PassphraseOptions field0)?  passphrase,}) {final _that = this;
switch (_that) {
case EntropyOptions_Password() when password != null:
return password(_that.field0);case EntropyOptions_Passphrase() when passphrase != null:
return passphrase(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class EntropyOptions_Password extends EntropyOptions {
  const EntropyOptions_Password(this.field0): super._();
  

@override final  PasswordOptions field0;

/// Create a copy of EntropyOptions
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$EntropyOptions_PasswordCopyWith<EntropyOptions_Password> get copyWith => _$EntropyOptions_PasswordCopyWithImpl<EntropyOptions_Password>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is EntropyOptions_Password&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'EntropyOptions.password(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $EntropyOptions_PasswordCopyWith<$Res> implements $EntropyOptionsCopyWith<$Res> {
  factory $EntropyOptions_PasswordCopyWith(EntropyOptions_Password value, $Res Function(EntropyOptions_Password) _then) = _$EntropyOptions_PasswordCopyWithImpl;
@useResult
$Res call({
 PasswordOptions field0
});




}
/// @nodoc
class _$EntropyOptions_PasswordCopyWithImpl<$Res>
    implements $EntropyOptions_PasswordCopyWith<$Res> {
  _$EntropyOptions_PasswordCopyWithImpl(this._self, this._then);

  final EntropyOptions_Password _self;
  final $Res Function(EntropyOptions_Password) _then;

/// Create a copy of EntropyOptions
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(EntropyOptions_Password(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as PasswordOptions,
  ));
}


}

/// @nodoc


class EntropyOptions_Passphrase extends EntropyOptions {
  const EntropyOptions_Passphrase(this.field0): super._();
  

@override final  PassphraseOptions field0;

/// Create a copy of EntropyOptions
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$EntropyOptions_PassphraseCopyWith<EntropyOptions_Passphrase> get copyWith => _$EntropyOptions_PassphraseCopyWithImpl<EntropyOptions_Passphrase>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is EntropyOptions_Passphrase&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'EntropyOptions.passphrase(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $EntropyOptions_PassphraseCopyWith<$Res> implements $EntropyOptionsCopyWith<$Res> {
  factory $EntropyOptions_PassphraseCopyWith(EntropyOptions_Passphrase value, $Res Function(EntropyOptions_Passphrase) _then) = _$EntropyOptions_PassphraseCopyWithImpl;
@useResult
$Res call({
 PassphraseOptions field0
});




}
/// @nodoc
class _$EntropyOptions_PassphraseCopyWithImpl<$Res>
    implements $EntropyOptions_PassphraseCopyWith<$Res> {
  _$EntropyOptions_PassphraseCopyWithImpl(this._self, this._then);

  final EntropyOptions_Passphrase _self;
  final $Res Function(EntropyOptions_Passphrase) _then;

/// Create a copy of EntropyOptions
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(EntropyOptions_Passphrase(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as PassphraseOptions,
  ));
}


}

// dart format on
