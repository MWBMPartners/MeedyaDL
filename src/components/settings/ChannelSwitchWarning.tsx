// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file ChannelSwitchWarning.tsx -- Confirmation modal shown before the user
 * subscribes to any pre-release update channel.
 *
 * Triggered when the user changes the Update Channel dropdown in
 * `GeneralTab.tsx` from `stable` (or any more-stable channel) to a
 * less-stable channel. The modal lists the concrete risks of running
 * pre-release software in technical (not consumer-friendly) language.
 *
 * The modal is **controlled** — its visibility is owned by the parent
 * (GeneralTab), which retains the previously-selected channel in local
 * state and only commits the change to the settings store after the user
 * clicks "Continue on {Channel}".
 *
 * The four most-experimental channels (Nightly, Weekly, Monthly, Alpha)
 * are gated behind Dev Access in the dropdown, so this warning is the
 * second line of defence for users who explicitly enabled Dev Access
 * and are now selecting one of those channels.
 */
import { Modal } from '../common/Modal';
import type { UpdateChannel } from '@/types';

const CHANNEL_LABELS: Record<UpdateChannel, string> = {
  nightly: 'Nightly',
  weekly: 'Weekly',
  monthly: 'Monthly',
  alpha: 'Alpha',
  beta: 'Beta',
  rc: 'Release Candidate',
  stable: 'Stable',
};

interface Props {
  open: boolean;
  targetChannel: UpdateChannel;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Pre-release stability warning modal. Shown before committing a switch
 * from the Stable channel (or any more-stable channel) to a less-stable
 * channel. The user must explicitly accept the listed risks to proceed.
 */
export default function ChannelSwitchWarning({
  open,
  targetChannel,
  onConfirm,
  onCancel,
}: Props) {
  const channelLabel = CHANNEL_LABELS[targetChannel];

  return (
    <Modal
      open={open}
      onClose={onCancel}
      title={`Switching to a pre-release channel (${channelLabel})`}
      maxWidth="max-w-xl"
    >
      <div className="space-y-4 text-sm text-content-secondary">
        <p>
          The <strong className="text-content-primary">{channelLabel}</strong> channel ships
          builds before they have completed full release validation. By continuing you accept
          the following:
        </p>

        <ul className="list-disc pl-5 space-y-2">
          <li>
            <strong className="text-content-primary">Bugs and regressions are expected.</strong>{' '}
            Pre-release builds are made available for testing precisely because they may contain
            defects that have not yet been identified or fixed.
          </li>
          <li>
            <strong className="text-content-primary">Data loss is possible.</strong> Settings,
            queues, and downloads may be corrupted by a release in this channel. Back up your
            MeedyaDL data directory before continuing.
          </li>
          <li>
            <strong className="text-content-primary">No guaranteed update path.</strong>{' '}
            Pre-release builds may be superseded, withdrawn, or never reach Stable. There is no
            guarantee that a future Stable release will be backwards-compatible with state
            written by this channel.
          </li>
          <li>
            <strong className="text-content-primary">No support obligation.</strong> Bug reports
            against pre-release builds are welcome but not prioritised against Stable defects.
          </li>
        </ul>

        <p className="text-xs text-content-tertiary">
          You can switch back to Stable at any time. The installer refuses to apply updates from
          a less-stable channel than your current selection, so moving up the ladder is always a
          deliberate action.
        </p>

        <div className="flex justify-end gap-3 pt-2">
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium bg-surface-secondary text-content-primary hover:opacity-90 transition-opacity border border-border"
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium bg-status-warning text-white hover:opacity-90 transition-opacity"
            onClick={onConfirm}
          >
            Continue on {channelLabel}
          </button>
        </div>
      </div>
    </Modal>
  );
}
