/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Settings page container.
 * Renders a tabbed interface with 9 settings categories.
 * Manages tab navigation, settings load/save, and the unsaved changes indicator.
 */

import { useEffect, useState } from 'react';
import {
  Settings as SettingsIcon,
  Music,
  ArrowDownUp,
  FolderOpen,
  Cookie,
  FileText,
  Image,
  Code,
  Wrench,
  Save,
  RotateCcw,
} from 'lucide-react';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';
import { Button } from '@/components/common';
import { PageHeader } from '@/components/layout';
import { GeneralTab } from './tabs/GeneralTab';
import { QualityTab } from './tabs/QualityTab';
import { FallbackTab } from './tabs/FallbackTab';
import { PathsTab } from './tabs/PathsTab';
import { CookiesTab } from './tabs/CookiesTab';
import { LyricsTab } from './tabs/LyricsTab';
import { CoverArtTab } from './tabs/CoverArtTab';
import { TemplatesTab } from './tabs/TemplatesTab';
import { AdvancedTab } from './tabs/AdvancedTab';

/** Tab definition with id, label, icon, and content component */
interface SettingsTab {
  id: string;
  label: string;
  icon: typeof SettingsIcon;
  component: React.FC;
}

/** All 9 settings tabs in display order */
const TABS: SettingsTab[] = [
  { id: 'general', label: 'General', icon: SettingsIcon, component: GeneralTab },
  { id: 'quality', label: 'Quality', icon: Music, component: QualityTab },
  { id: 'fallback', label: 'Fallback', icon: ArrowDownUp, component: FallbackTab },
  { id: 'paths', label: 'Paths', icon: FolderOpen, component: PathsTab },
  { id: 'cookies', label: 'Cookies', icon: Cookie, component: CookiesTab },
  { id: 'lyrics', label: 'Lyrics', icon: FileText, component: LyricsTab },
  { id: 'cover-art', label: 'Cover Art', icon: Image, component: CoverArtTab },
  { id: 'templates', label: 'Templates', icon: Code, component: TemplatesTab },
  { id: 'advanced', label: 'Advanced', icon: Wrench, component: AdvancedTab },
];

/**
 * Renders the full settings page with tabbed navigation on the left
 * and tab content on the right. Includes save/reset actions in the header.
 */
export function SettingsPage() {
  const [activeTab, setActiveTab] = useState('general');
  const loadSettings = useSettingsStore((s) => s.loadSettings);
  const saveSettings = useSettingsStore((s) => s.saveSettings);
  const resetToDefaults = useSettingsStore((s) => s.resetToDefaults);
  const isDirty = useSettingsStore((s) => s.isDirty);
  const isLoading = useSettingsStore((s) => s.isLoading);
  const addToast = useUiStore((s) => s.addToast);

  /* Load settings on mount */
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  /** Save settings to disk */
  const handleSave = async () => {
    try {
      await saveSettings();
      addToast('Settings saved successfully', 'success');
    } catch {
      addToast('Failed to save settings', 'error');
    }
  };

  /** Reset settings to defaults */
  const handleReset = () => {
    resetToDefaults();
    addToast('Settings reset to defaults', 'info');
  };

  /* Find the active tab's component */
  const ActiveComponent =
    TABS.find((t) => t.id === activeTab)?.component || GeneralTab;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Settings"
        subtitle="Configure download options, paths, and preferences"
        actions={
          <div className="flex gap-2">
            {/* Reset to defaults */}
            <Button
              variant="ghost"
              size="sm"
              icon={<RotateCcw size={14} />}
              onClick={handleReset}
            >
              Reset
            </Button>

            {/* Save button (shows unsaved indicator) */}
            <Button
              variant="primary"
              size="sm"
              icon={<Save size={14} />}
              onClick={handleSave}
              disabled={!isDirty || isLoading}
            >
              {isDirty ? 'Save Changes' : 'Saved'}
            </Button>
          </div>
        }
      />

      {/* Tab navigation + content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Tab sidebar */}
        <nav className="w-44 flex-shrink-0 border-r border-border-light overflow-y-auto p-2 space-y-0.5">
          {TABS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActiveTab(id)}
              className={`
                w-full flex items-center gap-2.5 px-3 py-2
                rounded-platform text-sm transition-colors
                ${
                  activeTab === id
                    ? 'bg-accent-light text-accent font-medium'
                    : 'text-content-secondary hover:text-content-primary hover:bg-surface-secondary'
                }
              `}
            >
              <Icon size={16} className="flex-shrink-0" />
              {label}
            </button>
          ))}
        </nav>

        {/* Tab content */}
        <div className="flex-1 overflow-y-auto p-6">
          <ActiveComponent />
        </div>
      </div>
    </div>
  );
}
