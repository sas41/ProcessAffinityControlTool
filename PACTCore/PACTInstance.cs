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
        //public event ConfigUpdated OnConfigUpdated;
        public event ConfigUpdatedEventHandler ConfigUpdated;

        public ProcessOverwatch PACTProcessOverwatch { get; private set; }
        // The active config is public so that it can be changed.
        public PACTConfig ActivePACTConfig { get; private set; }
        public PACTConfig PausedPACTConfig { get; private set; }
        public bool IsActive { get; private set; }



        public PACTInstance()
        {
            PACTProcessOverwatch = new ProcessOverwatch();
            ReadConfig();
            PausedPACTConfig = new PACTConfig();
            IsActive = false;
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
            IsActive = !(IsActive);
            if (IsActive)
            {
                // If we just switched from false to true...
                PACTProcessOverwatch.Config = ActivePACTConfig;
                PACTProcessOverwatch.RunScan(true);
                PACTProcessOverwatch.SetTimer();
            }
            else
            {
                // If we just switched from true to false...
                PACTProcessOverwatch.PauseTimer();
                PACTProcessOverwatch.Config = PausedPACTConfig;
                PACTProcessOverwatch.RunScan(true);
            }
            OnConfigUpdated();
            return IsActive;
        }

        private void ReadConfig()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "/Config/config.json";

            if (File.Exists(configPath))
            {
                string json = File.ReadAllText(configPath);
                PACTConfig tconf = JsonSerializer.Deserialize<PACTConfig>(json);
                ActivePACTConfig = tconf;
                PACTProcessOverwatch.RunScan(true);
            }
            else
            {
                ActivePACTConfig = new PACTConfig();
                SaveConfig();
            }

            OnConfigUpdated();
        }

        private void SaveConfig()
        {
            string json = JsonSerializer.Serialize<PACTConfig>(ActivePACTConfig, new JsonSerializerOptions { WriteIndented = true });
            string path = AppDomain.CurrentDomain.BaseDirectory + "/Config/";
            (new FileInfo(path)).Directory.Create();
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);
            PACTProcessOverwatch.RunScan(true);
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
                ActivePACTConfig = tconf;
                PACTProcessOverwatch.RunScan(true);
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
            string json = JsonSerializer.Serialize<PACTConfig>(ActivePACTConfig, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(fullpath, json);
        }

        public void ImportHighPerformance(string fullpath)
        {
            BackupConfig("BeforeHighPerformanceImport");
            ImportFile(fullpath, ActivePACTConfig.AddToHighPerformance);
            OnConfigUpdated();
            SaveConfig();
        }

        public void ImportBlacklist(string fullpath)
        {
            BackupConfig("BeforeBlackListImport");
            ImportFile(fullpath, ActivePACTConfig.AddToBlacklist);
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
                PACTProcessOverwatch.RunScan(true);
            }
            else
            {
                throw new FileLoadException("Specified File Does not exist!");
            }
        }

        public void ExportHighPerformance(string fullpath)
        {
            File.WriteAllLines(fullpath, ActivePACTConfig.HighPerformanceProcesses);
        }

        public void ExportBlacklist(string fullpath)
        {
            File.WriteAllLines(fullpath, ActivePACTConfig.Blacklist);
        }

        public void ClearHighPerformance()
        {
            BackupConfig("BeforeClearHighPerformance");

            ActivePACTConfig.HighPerformanceProcesses.Clear();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ClearCustoms()
        {
            BackupConfig("BeforeClearCustoms");

            ActivePACTConfig.CustomPerformanceProcesses.Clear();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ClearBlackList()
        {
            BackupConfig("BeforeClearBlacklist");

            ActivePACTConfig.Blacklist.Clear();

            OnConfigUpdated();
            SaveConfig();
        }

        public void ResetConfig()
        {
            BackupConfig("BeforeResetConfig");

            ActivePACTConfig = new PACTConfig();
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
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
            return ActivePACTConfig.HighPerformanceProcesses.OrderBy(x => x).ToList();
        }

        public IReadOnlyList<string> GetCustomProcesses()
        {
            return ActivePACTConfig.CustomPerformanceProcesses.Keys.OrderBy(x => x).ToList();
        }

        public IReadOnlyList<string> GetBlacklistedProcesses()
        {
            return ActivePACTConfig.Blacklist.OrderBy(x => x).ToList();
        }


        public void AddToBlacklist(string name)
        {
            ActivePACTConfig.AddToBlacklist(name);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToBlacklist(List<string> names)
        {
            foreach (var name in names)
            {
                ActivePACTConfig.AddToBlacklist(name);
            }
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToHighPerformance(string name)
        {
            ActivePACTConfig.AddToHighPerformance(name);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToHighPerformance(List<string> names)
        {
            foreach (var name in names)
            {
                ActivePACTConfig.AddToHighPerformance(name);
            }
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToCustomPriority(string name, ProcessConfig conf)
        {
            ActivePACTConfig.AddOToCustomPerformance(name, conf);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void AddToCustomPriority(List<string> names, ProcessConfig conf)
        {
            foreach (var name in names)
            {
                ActivePACTConfig.AddOToCustomPerformance(name, conf);
            }
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void ClearProcesses(List<string> names)
        {
            foreach (var name in names)
            {
                ActivePACTConfig.ClearProcessConfig(name);
            }
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void UpdateHighPerformanceProcessConfig(ProcessConfig conf)
        {
            BackupConfig("BeforeUpdateHighPerformanceConfig");
            ActivePACTConfig.HighPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public void UpdateDefaultPriorityProcessConfig(ProcessConfig conf)
        {
            BackupConfig("BeforeUpdateNormalPerformanceConfig");
            ActivePACTConfig.DefaultPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }
    }
}
