import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/reveal.dart';
import '../../state/vault_actions.dart';
import '../../state/vault_query.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../shell/adaptive_shell.dart';
import '../widgets/l10n_ext.dart';
import 'item_detail_pane.dart';
import 'item_list_pane.dart';
import 'vault_sidebar.dart';

/// The unlocked app: desktop three panes (sidebar, list, detail) or the compact
/// list -> detail layout (docs/07 §8). Also hides revealed secrets when the app
/// loses focus (B02 item 3).
class VaultShell extends ConsumerStatefulWidget {
  const VaultShell({super.key});

  @override
  ConsumerState<VaultShell> createState() => _VaultShellState();
}

class _VaultShellState extends ConsumerState<VaultShell>
    with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) {
      ref.read(revealProvider.notifier).hideAll();
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    // Keep the vault settings (reveal and clipboard timings) loaded.
    ref.watch(appSettingsProvider);
    final String? selected = ref.watch(
      vaultQueryProvider.select((VaultQueryState s) => s.selectedId),
    );
    final Widget lock = IconButton(
      tooltip: l10n.actionLock,
      icon: const Icon(Icons.lock),
      onPressed: () => ref.read(vaultActionsProvider).lock(),
    );
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final bool desktop = constraints.maxWidth >= kDesktopBreakpoint;
        if (desktop) {
          return Scaffold(
            appBar: AppBar(
              title: Semantics(header: true, child: Text(l10n.homeTitle)),
              actions: <Widget>[lock, const SizedBox(width: 8)],
            ),
            body: Row(
              children: <Widget>[
                SizedBox(
                  width: 240,
                  child: FocusTraversalGroup(child: const VaultSidebar()),
                ),
                const VerticalDivider(width: 1),
                SizedBox(
                  width: 380,
                  child: FocusTraversalGroup(child: const ItemListPane()),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: FocusTraversalGroup(
                    child: selected == null
                        ? Center(
                            child: Padding(
                              padding: EdgeInsets.all(tokens.gap),
                              child: Text(
                                l10n.selectItemHint,
                                textAlign: TextAlign.center,
                              ),
                            ),
                          )
                        : ItemDetailPane(
                            key: ValueKey<String>(selected),
                            itemId: selected,
                          ),
                  ),
                ),
              ],
            ),
          );
        }
        return PopScope(
          canPop: selected == null,
          onPopInvokedWithResult: (bool didPop, Object? result) {
            if (!didPop) ref.read(vaultQueryProvider.notifier).select(null);
          },
          child: Scaffold(
            appBar: AppBar(
              automaticallyImplyLeading: false,
              leading: selected != null
                  ? IconButton(
                      tooltip: l10n.actionBack,
                      icon: const Icon(Icons.arrow_back),
                      onPressed: () =>
                          ref.read(vaultQueryProvider.notifier).select(null),
                    )
                  : Builder(
                      builder: (BuildContext context) => IconButton(
                        tooltip: l10n.openMenu,
                        icon: const Icon(Icons.menu),
                        onPressed: () => Scaffold.of(context).openDrawer(),
                      ),
                    ),
              title: Semantics(header: true, child: Text(l10n.homeTitle)),
              actions: <Widget>[lock],
            ),
            drawer: Drawer(
              child: SafeArea(
                child: Builder(
                  builder: (BuildContext context) => VaultSidebar(
                    onNavigate: () => Navigator.of(context).maybePop(),
                  ),
                ),
              ),
            ),
            body: SafeArea(
              child: selected == null
                  ? const ItemListPane()
                  : ItemDetailPane(
                      key: ValueKey<String>(selected),
                      itemId: selected,
                    ),
            ),
          ),
        );
      },
    );
  }
}
