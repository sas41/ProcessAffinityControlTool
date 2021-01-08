using System;
using System.Collections.Generic;
using System.Text;
using System.IO;
using System.Diagnostics;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PACTCore
{
    // Don't do or use this, I abstracted and automated this class so much, it's stiff as a dead mule.
    // I wanted to just create a single line solution so I can focus on the UI part of the code.
    public class PACTInstance
    {
        public delegate void ConfigUpdatedEventHandler(object sender, EventArgs e);
        public event ConfigUpdatedEventHandler ConfigUpdated;

        public ProcessOverwatch PACTProcessOverwatch { get; private set; }

        public PACTInstance()
        {
            PACTProcessOverwatch = new ProcessOverwatch(ReadConfig());
            PACTProcessOverwatch.RequestFreshScan();
            SaveConfig();
        }

        protected virtual void OnConfigUpdated(string updateReason = "")
        {
            if (ConfigUpdated != null)
            {
                ConfigUpdated(this, EventArgs.Empty);
            }
        }

        public bool ToggleProcessOverwatch()
        {
            bool state = PACTProcessOverwatch.ToggleProcessOverwatch();
            OnConfigUpdated();
            return state;
        }

        private PACTConfig ReadConfig()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "/Config/config.json";
            PACTConfig tconf = new PACTConfig();

            if (File.Exists(configPath))
            {
                string json = File.ReadAllText(configPath);
                tconf = JsonSerializer.Deserialize<PACTConfig>(json);
            }

            return tconf;
        }

        public void SaveConfig()
        {
            string json = JsonSerializer.Serialize<PACTConfig>(PACTProcessOverwatch.UserConfig, new JsonSerializerOptions { WriteIndented = true });
            string path = AppDomain.CurrentDomain.BaseDirectory + "/Config/";
            (new FileInfo(path)).Directory.Create();
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);
        }

        private void BackupConfig(string reason)
        {
            string path = $"{AppDomain.CurrentDomain.BaseDirectory}/Config/Backups/{DateTime.Now.ToString("yyyy-MM-dd-HH-mm-ss")}-{reason}/";
            (new FileInfo(path)).Directory.Create();
            ExportConfig(path + "/config.json");
        }

        public void ImportConfig(string fullpath)
        {
            if (File.Exists(fullpath))
            {
                string json = File.ReadAllText(fullpath);
                PACTConfig tconf = JsonSerializer.Deserialize<PACTConfig>(json);
                PACTProcessOverwatch.UserConfig = tconf;
                PACTProcessOverwatch.RequestFreshScan();
            }
            else
            {
                throw new FileLoadException("Specified Config File Does not exist!");
            }

            OnConfigUpdated();
            SaveConfig();
        }

        public void ExportConfig(string fullpath)
        {
            string json = JsonSerializer.Serialize<PACTConfig>(PACTProcessOverwatch.UserConfig, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(fullpath, json);
        }

        public void ImportHighPerformance(string fullpath)
        {
            BackupConfig("BeforeHighPerformanceImport");
            ImportFile(fullpath, PACTProcessOverwatch.UserConfig.AddToHighPerformance);
            OnConfigUpdated();
            SaveConfig();
        }

        public void ImportBlacklist(string fullpath)
        {
            BackupConfig("BeforeBlackListImport");
            ImportFile(fullpath, PACTProcessOverwatch.UserConfig.AddToBlacklist);
            OnConfigUpdated();
            SaveConfig();
        }

        private void ImportFile(string fullpath, Action<string> func)
        {
            if (File.Exists(fullpath))
            {
                List<string> lines = File.ReadAllLines(fullpath).ToList();
                foreach (var line in lines)
                {
                    func(line);
                }
            }
            else
            {
                throw new FileLoadException("Specified File Does not exist!");
            }
        }

        public void ExportHighPerformance(string fullpath)
        {
            File.WriteAllLines(fullpath, PACTProcessOverwatch.UserConfig.HighPerformanceProcesses);
        }

        public void ExportBlacklist(string fullpath)
        {
            File.WriteAllLines(fullpath, PACTProcessOverwatch.UserConfig.Blacklist);
        }

        public void ClearHighPerformance()
        {
            BackupConfig("BeforeClearHighPerformance");

            PACTProcessOverwatch.UserConfig.HighPerformanceProcesses.Clear();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ClearCustoms()
        {
            BackupConfig("BeforeClearCustoms");

            PACTProcessOverwatch.UserConfig.CustomPerformanceProcesses.Clear();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ClearBlackList()
        {
            BackupConfig("BeforeClearBlacklist");

            PACTProcessOverwatch.UserConfig.Blacklist.Clear();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ResetConfig()
        {
            BackupConfig("BeforeResetConfig");

            PACTProcessOverwatch.UserConfig = new PACTConfig();
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }


        public ProcessConfig GetDefaultPerformanceConfig()
        {
            return PACTProcessOverwatch.UserConfig.DefaultPerformanceProcessConfig;
        }

        public ProcessConfig GetHighPerformanceConfig()
        {
            return PACTProcessOverwatch.UserConfig.HighPerformanceProcessConfig;
        }


        public IReadOnlyList<string> GetAllRunningProcesses()
        {
            // Todo: This is cursed.
            // Needs to be re-written so that it can return exe names
            return Process.GetProcesses()
                .Select(x => x.ProcessName)
                .OrderBy(x => x)
                .ToList() as IReadOnlyList<string>;
        }

        public IReadOnlyList<string> GetProtectedProcesses()
        {
            return PACTProcessOverwatch.ProtectedProcesses.Select(x => x.ProcessName).OrderBy(x => x).ToList() as IReadOnlyList<string>;
        }

        public IReadOnlyList<string> GetNormalPerformanceProcesses()
        {
            // What a mess

            var allProcesses = GetAllRunningProcesses().Distinct().ToList();
            
            var highPerformances = GetHighPerformanceProcesses().Select(x => x.ToLower()).ToList();
            var customPerformances = GetCustomProcesses().Select(x => x.ToLower()).ToList();
            var blacklisted = GetBlacklistedProcesses().Select(x => x.ToLower()).ToList();

            var completeSet = new List<string>();
            completeSet.AddRange(highPerformances);
            completeSet.AddRange(customPerformances);
            completeSet.AddRange(blacklisted);

            var filtered = allProcesses.Where(x => !completeSet.Contains(x.ToLower()));

            return filtered.ToList() as IReadOnlyList<string>;
        }

        public IReadOnlyList<string> GetHighPerformanceProcesses()
        {
            return PACTProcessOverwatch.UserConfig.HighPerformanceProcesses.OrderBy(x => x).ToList();
        }

        public IReadOnlyList<string> GetCustomProcesses()
        {
            return PACTProcessOverwatch.UserConfig.CustomPerformanceProcesses.Keys.OrderBy(x => x).ToList();
        }

        public IReadOnlyList<string> GetBlacklistedProcesses()
        {
            return PACTProcessOverwatch.UserConfig.Blacklist.OrderBy(x => x).ToList();
        }

        public IReadOnlyList<string> GetAutoModeLaunchers()
        {
            return PACTProcessOverwatch.UserConfig.AutoModeLaunchers.OrderBy(x => x).ToList();
        }

        public IReadOnlyList<string> GetAutoModeDetections()
        {
            return PACTProcessOverwatch.AutoModeDetections.OrderBy(x => x).ToList();
        }

        public void RemoveFromAutoModeLaunchers(string name)
        {
            PACTProcessOverwatch.UserConfig.RemoveFromAutoModeLaunchers(name);
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToAutoModeLaunchers(string name)
        {
            PACTProcessOverwatch.UserConfig.AddToAutoModeLaunchers(name);
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToBlacklist(string name)
        {
            PACTProcessOverwatch.UserConfig.AddToBlacklist(name);
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToBlacklist(List<string> names)
        {
            foreach (var name in names)
            {
                PACTProcessOverwatch.UserConfig.AddToBlacklist(name);
            }
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToHighPerformance(string name)
        {
            PACTProcessOverwatch.UserConfig.AddToHighPerformance(name);
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToHighPerformance(List<string> names)
        {
            foreach (var name in names)
            {
                PACTProcessOverwatch.UserConfig.AddToHighPerformance(name);
            }
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToCustomPriority(string name, ProcessConfig conf)
        {
            PACTProcessOverwatch.UserConfig.AddOToCustomPerformance(name, conf);
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToCustomPriority(List<string> names, ProcessConfig conf)
        {
            foreach (var name in names)
            {
                PACTProcessOverwatch.UserConfig.AddOToCustomPerformance(name, conf);
            }
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ClearProcesses(List<string> names)
        {
            foreach (var name in names)
            {
                PACTProcessOverwatch.UserConfig.ClearProcessConfig(name);
            }
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void UpdateHighPerformanceProcessConfig(ProcessConfig conf)
        {
            BackupConfig("BeforeUpdateHighPerformanceConfig");
            PACTProcessOverwatch.UserConfig.HighPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public void UpdateDefaultPerformanceProcessConfig(ProcessConfig conf)
        {
            BackupConfig("BeforeUpdateNormalPerformanceConfig");
            PACTProcessOverwatch.UserConfig.DefaultPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RequestFreshScan();

            OnConfigUpdated();
            SaveConfig();
        }

        public bool ToggleAutoMode()
        {
            return PACTProcessOverwatch.ToggleAutoMode();
        }
    }
}
