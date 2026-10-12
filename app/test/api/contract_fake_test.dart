import 'package:arya_vault/src/api/vault_api.dart';

import '../helpers.dart';
import 'contract_suite.dart';

void main() {
  runContractSuite('FakeVaultApi', () async => newFakeApi() as VaultApi);
}
