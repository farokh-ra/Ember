package io.nexus.ui.components

import io.nexus.presentation.demo.BootstrapPathState
import io.nexus.presentation.demo.DemoCategory
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.NexusStrings

/** Localized category name for demo fixtures. */
public fun demoCategoryLabel(
    category: DemoCategory,
    strings: NexusStrings,
): String =
    when (category) {
        DemoCategory.DOCUMENTS -> strings.categoryDocuments
        DemoCategory.MEDIA -> strings.categoryMedia
        DemoCategory.ARCHIVES -> strings.categoryArchives
        DemoCategory.OTHER -> strings.categoryOther
    }

/** Localized bootstrap-path state. */
public fun bootstrapStateLabel(
    state: BootstrapPathState,
    strings: NexusStrings,
): String =
    when (state) {
        BootstrapPathState.CONNECTED -> strings.networkPathConnected
        BootstrapPathState.CONNECTING -> strings.networkPathConnecting
        BootstrapPathState.UNREACHABLE -> strings.networkPathUnreachable
    }

/** Semantic tone for a bootstrap path. */
public fun bootstrapStateTone(state: BootstrapPathState): StatusTone =
    when (state) {
        BootstrapPathState.CONNECTED -> StatusTone.AVAILABLE
        BootstrapPathState.CONNECTING -> StatusTone.INFO
        BootstrapPathState.UNREACHABLE -> StatusTone.NEGATIVE
    }

/** Icon for a bootstrap path state. */
internal fun bootstrapStateIcon(state: BootstrapPathState) =
    when (state) {
        BootstrapPathState.CONNECTED -> NexusIcons.Check
        BootstrapPathState.CONNECTING -> NexusIcons.Refresh
        BootstrapPathState.UNREACHABLE -> NexusIcons.Error
    }
