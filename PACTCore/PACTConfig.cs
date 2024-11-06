using System;
using System.Text;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PACTCore
{
    public class PACTConfig
    {
        // Set of EXE names with their corresponding process configs.
        [JsonInclude]
        public CaseInsensitiveDictionary<ProcessConfig> CustomPerformanceProcesses { get; set; }

        // Set of high performance executable names.
        [JsonInclude]
        public CaseInsensitiveHashSet HighPerformanceProcesses { get; set; }

        // Set of blacklisted executable names.
        [JsonInclude]
        public CaseInsensitiveHashSet Blacklist { get; set; }

        // Set of launchers whose child processes get automatically added to High Priority.
        [JsonInclude]
        public CaseInsensitiveHashSet AutoModeLaunchers { get; set; }

        // Applies to all high-performance processes.
        [JsonInclude]
        public ProcessConfig HighPerformanceProcessConfig { get; set; }

        // Applies to all non-high-performance AND non-custom-performance processes.
        [JsonInclude]
        public ProcessConfig DefaultPerformanceProcessConfig { get; set; }

        [JsonInclude]
        public int ScanInterval { get; set; }

        // Store user preference about starting the app at startup.
        [JsonInclude]
        public bool StartWithWindows { get; set; }

        public PACTConfig()
        {
            CustomPerformanceProcesses = new CaseInsensitiveDictionary<ProcessConfig>();
            HighPerformanceProcesses = new CaseInsensitiveHashSet();
            Blacklist = new CaseInsensitiveHashSet();
            AutoModeLaunchers = new CaseInsensitiveHashSet();
            HighPerformanceProcessConfig = new ProcessConfig();
            DefaultPerformanceProcessConfig = new ProcessConfig();

            ScanInterval = 3000;

            // This method of seeding is not the best.
            // I will likely change it at some point.
            AutoModeLaunchers.Add("Battle.net");
            AutoModeLaunchers.Add("Battle.net Launcher");
            AutoModeLaunchers.Add("EALink");
            AutoModeLaunchers.Add("EpicGamesLauncher");
            AutoModeLaunchers.Add("GalaxyClient");
            AutoModeLaunchers.Add("GalaxyClientService");
            AutoModeLaunchers.Add("Origin");
            AutoModeLaunchers.Add("OriginClientService");
            AutoModeLaunchers.Add("steam");
            AutoModeLaunchers.Add("steamservice");
            AutoModeLaunchers.Add("UbisoftGameLauncher");
            AutoModeLaunchers.Add("UbisoftGameLauncher64");
            AutoModeLaunchers.Add("UnrealEngineLauncher");
            AutoModeLaunchers.Add("Uplay");
            AutoModeLaunchers.Add("UplayService");
            AutoModeLaunchers.Add("x64launcher");
            AutoModeLaunchers.Add("x86launcher");
            
            // Apply startup setting
            UpdateStartupSetting();
        }

        // Method to add or remove the app from startup based on the `StartWithWindows` value
        public void UpdateStartupSetting()
        {
            const string appName = "PACTApp";
            string appPath = System.Diagnostics.Process.GetCurrentProcess().MainModule.FileName;

            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run", true))
            {
                if (StartWithWindows)
                {
                    key.SetValue(appName, appPath);
                }
                else if (key.GetValue(appName) != null)
                {
                    key.DeleteValue(appName);
                }
            }
        }

        // Call this method when changing the StartWithWindows setting at runtime
        public void SetStartWithWindows(bool startWithWindows)
        {
            StartWithWindows = startWithWindows;
            UpdateStartupSetting();
        }


        public void RecalculateAffinities()
        {
            DefaultPerformanceProcessConfig.ReCalculateMask();
            HighPerformanceProcessConfig.ReCalculateMask();

            foreach (var kvp in CustomPerformanceProcesses)
            {
                kvp.Value.ReCalculateMask();
            }
        }

        public void AddOToCustomPerformance(string name, ProcessConfig conf)
        {
            if (!string.IsNullOrEmpty(name))
            {
                ClearProcessConfig(name);
                CustomPerformanceProcesses.Add(name, conf);
            }
        }

        public void AddToHighPerformance(string name)
        {
            if (!string.IsNullOrEmpty(name))
            {
                ClearProcessConfig(name);
                HighPerformanceProcesses.Add(name);
            }
        }

        public void AddToBlacklist(string name)
        {
            if (!string.IsNullOrEmpty(name))
            {
                ClearProcessConfig(name);
                Blacklist.Add(name);
            }
        }

        public void AddToAutoModeLaunchers(string name)
        {
            if (!string.IsNullOrEmpty(name))
            {
                AutoModeLaunchers.Add(name);
            }
        }

        public void RemoveFromAutoModeLaunchers(string name)
        {
            if (!string.IsNullOrEmpty(name) && AutoModeLaunchers.Contains(name))
            {
                AutoModeLaunchers.Remove(name);
            }
        }

        public void ClearProcessConfig(string name)
        {
            if (CustomPerformanceProcesses.ContainsKey(name))
            {
                CustomPerformanceProcesses.Remove(name);
            }

            if (HighPerformanceProcesses.Contains(name))
            {
                HighPerformanceProcesses.Remove(name);
            }

            if (Blacklist.Contains(name))
            {
                Blacklist.Remove(name);
            }
        }
    }
}
