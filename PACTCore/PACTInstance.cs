using System;
using System.Collections.Generic;
using System.Text;
using Newtonsoft.Json;
using System.IO;
using System.Diagnostics;
using System.Linq;

namespace PACTCore
{
    // Don't do or use this, I abstracted and automated this class so much, it's stiff as a dead mule.
    // I wanted to just create a single line solution so I can focus on the UI part of the code.
    public class PACTInstance
    {

        public delegate void ConfigUpdatedEventHandler(object sender, EventArgs e);
        //public  event ConfigUpdated OnConfigUpdated;
        public  event ConfigUpdatedEventHandler ConfigUpdated;

        private ProcessOverwatch PACTProcessOverwatch { get; set; }
        // The active config is public so that it can be changed.
        public PACTConfig ActivePACTConfig { get; set; }
        private PACTConfig PausedPACTConfig { get; set; }
        private bool IsActive { get; set; }



        public PACTInstance()
        {
            PACTProcessOverwatch = new ProcessOverwatch();
            ReadConfig();
            PausedPACTConfig = new PACTConfig();
            IsActive = false;
        }

        protected  virtual void OnConfigUpdated(string updateReason = "")
        {
            if (ConfigUpdated != null)
            {
                ConfigUpdated(this, EventArgs.Empty);
            }
        }


        public  bool ToggleProcessOverwatch()
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

        public  void ReadConfig()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "/Config/config.json";
            string json = "";

            if (File.Exists(configPath))
            {
                json = File.ReadAllText(configPath);
                ActivePACTConfig = JsonConvert.DeserializeObject<PACTConfig>(json);
                return;
            }

            ActivePACTConfig = new PACTConfig();

            OnConfigUpdated();
        }

        public  void SaveConfig()
        {
            string json = JsonConvert.SerializeObject(ActivePACTConfig, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory + "/Config/";
            (new FileInfo(path)).Directory.Create();
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);
            PACTProcessOverwatch.RunScan(true);
        }

        private  void BackupConfig(string reason)
        {
            string path = $"{AppDomain.CurrentDomain.BaseDirectory}/Config/Backups/{DateTime.Now.ToString("yyyy-MM-dd-HH-mm-ss")}-{reason}/";
            (new FileInfo(path)).Directory.Create();
            ExportConfig(path + "/config.json");
        }

        public  void ImportConfig(string fullpath)
        {
            if (File.Exists(fullpath))
            {
                string json = File.ReadAllText(fullpath);
                PACTConfig tconf = JsonConvert.DeserializeObject<PACTConfig>(json);
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

        public  void ExportConfig(string fullpath)
        {
            string json = JsonConvert.SerializeObject(ActivePACTConfig, Formatting.Indented);
            File.WriteAllText(fullpath, json);
        }

        public  void ImportHighPerformance(string fullpath)
        {
            BackupConfig("BeforeHighPerformanceImport");
            if (File.Exists(fullpath))
            {
                List<string> lines = File.ReadAllLines(fullpath).ToList();
                foreach (var line in lines)
                {
                    ActivePACTConfig.AddOrUpdate(line);
                }
                PACTProcessOverwatch.RunScan(true);
            }
            else
            {
                throw new FileLoadException("Specified File Does not exist!");
            }

            OnConfigUpdated();
            SaveConfig();
        }

        public  void ExportHighPerformance(string fullpath)
        {
            File.WriteAllLines(fullpath, ActivePACTConfig.GetHighPerformanceProcesses());
        }

        public  void ImportBlacklist(string fullpath)
        {
            BackupConfig("BeforeBlackListImport");

            if (File.Exists(fullpath))
            {
                List<string> lines = File.ReadAllLines(fullpath).ToList();
                foreach (var line in lines)
                {
                    ActivePACTConfig.AddToBlacklist(line);
                }
            }
            else
            {
                throw new FileLoadException("Specified File Does not exist!");
            }

            OnConfigUpdated();
            SaveConfig();
        }

        public  void ExportBlacklist(string fullpath)
        {
            File.WriteAllLines(fullpath, ActivePACTConfig.GetBlacklistedProcesses());
        }

        public  void ClearHighPerformance()
        {
            BackupConfig("BeforeClearHighPerformance");
            var list = GetHighPerformanceProcesses();
            ActivePACTConfig.ClearHighPerformanceProcessList();

            OnConfigUpdated();
            SaveConfig();
        }

        public  void ClearCustoms()
        {
            BackupConfig("BeforeClearCustoms");
            var list = GetCustomProcesses();
            ActivePACTConfig.ClearCustomPerformanceProcessList();

            OnConfigUpdated();
            SaveConfig();
        }

        public  void ClearBlackList()
        {
            BackupConfig("BeforeClearBlacklist");
            var list = GetBlacklistedProcesses();
            ActivePACTConfig.ClearBlacklist();

            OnConfigUpdated();
            SaveConfig();
        }

        public  void ResetConfig()
        {
            BackupConfig("BeforeResetConfig");
            ActivePACTConfig = new PACTConfig();
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }




        public  IReadOnlyList<string> GetAllRunningProcesses()
        {
            // Todo: This is cursed.
            // Needs to be re-written so that it can return exe names
            return Process.GetProcesses()
                .Select(x => x.ProcessName)
                .OrderBy(x => x)
                .ToList() as IReadOnlyList<string>;
        }

        public  IReadOnlyList<string> GetProtectedProcesses()
        {
            return PACTProcessOverwatch.ProtectedProcesses.Select(x => x.ProcessName).OrderBy(x => x).ToList() as IReadOnlyList<string>;
        }

        public  IReadOnlyList<string> GetNormalPerformanceProcesses()
        {
            // What a mess

            var allProcesses = GetAllRunningProcesses().Distinct().ToList();
            var normalizedDistinct = GetAllRunningProcesses().Select(x => x.ToLower()).Distinct().ToList();
            var highPerformances = GetHighPerformanceProcesses().Select(x => x.ToLower()).ToList();
            var customPerformances = GetCustomProcesses().Select(x => x.ToLower()).ToList();
            var blacklisted = GetBlacklistedProcesses().Select(x => x.ToLower()).ToList();

            var filtered = allProcesses.Where(x => (highPerformances.Contains(x.ToLower()) || customPerformances.Contains(x.ToLower()) || blacklisted.Contains(x.ToLower())) == false );

            return filtered.ToList() as IReadOnlyList<string>;
        }

        public  IReadOnlyList<string> GetHighPerformanceProcesses()
        {
            return PACTProcessOverwatch.Config.GetHighPerformanceProcesses().OrderBy(x => x).ToList();
        }

        public  IReadOnlyList<string> GetCustomProcesses()
        {
            return PACTProcessOverwatch.Config.GetCustomPerformanceProcessConfigs().OrderBy(x => x).ToList();
        }

        public  IReadOnlyList<string> GetBlacklistedProcesses()
        {
            return PACTProcessOverwatch.Config.GetBlacklistedProcesses().OrderBy(x => x).ToList();
        }

        public  IReadOnlyList<int> GetHighPerformanceCores()
        {
            return PACTProcessOverwatch.Config.HighPerformanceProcessConfig.CoreList;
        }
        public  IReadOnlyList<int> GetNormalPerformanceCores()
        {
            return PACTProcessOverwatch.Config.DefaultPerformanceProcessConfig.CoreList;
        }




        public  void AddToBlacklist(string name)
        {
            ActivePACTConfig.AddToBlacklist(name);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public  void AddToHighPerformance(string name)
        {
            ActivePACTConfig.AddOrUpdate(name);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public  void AddToCustomPriority(string name, ProcessConfig conf)
        {
            ActivePACTConfig.AddOrUpdate(name, conf);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public  void ClearProcess(string name)
        {
            ActivePACTConfig.ClearProcessConfig(name);
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public  void UpdateHighPerformanceProcessConfig(ProcessConfig conf)
        {
            BackupConfig("BeforeUpdateHighPerformanceConfig");
            ActivePACTConfig.HighPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }

        public  void UpdateDefaultPriorityProcessConfig(ProcessConfig conf)
        {
            BackupConfig("BeforeUpdateNormalPerformanceConfig");
            ActivePACTConfig.DefaultPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);

            OnConfigUpdated();
            SaveConfig();
        }
    }
}
