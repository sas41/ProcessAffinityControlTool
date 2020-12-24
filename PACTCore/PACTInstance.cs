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
    public static class PACTInstance
    {
        private static ProcessOverwatch PACTProcessOverwatch { get; set; }
        // The active config is public so that it can be changed.
        public static PACTConfig ActivePACTConfig { get; set; }
        private static PACTConfig PausedPACTConfig { get; set; }
        private static bool IsActive { get; set; }



        static PACTInstance()
        {
            PACTProcessOverwatch = new ProcessOverwatch();
            ReadConfig();
            PausedPACTConfig = new PACTConfig();
            IsActive = false;
        }



        public static bool ToggleProcessOverwatch()
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

            return IsActive;
        }

        public static void ReadConfig()
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

        }

        public static void SaveConfig()
        {
            string json = JsonConvert.SerializeObject(ActivePACTConfig, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory + "/Config/";
            (new FileInfo(path)).Directory.Create();
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);
            PACTProcessOverwatch.RunScan(true);
        }

        private static void BackupConfig()
        {
            string path = $"{AppDomain.CurrentDomain.BaseDirectory}/Config/Backups/{DateTime.Now.ToString("yyyy-MM-dd-HH-mm-ss")}/";
            (new FileInfo(path)).Directory.Create();
            ExportConfig(path + "/config.json");
        }

        public static void ImportConfig(string fullpath)
        {
            if (File.Exists(fullpath))
            {
                string json = File.ReadAllText(fullpath);
                PACTConfig tconf = JsonConvert.DeserializeObject<PACTConfig>(json);
                ActivePACTConfig = tconf;
            }
            else
            {
                throw new FileLoadException("Specified Config File Does not exist!");
            }

            SaveConfig();
        }

        public static void ExportConfig(string fullpath)
        {
            string json = JsonConvert.SerializeObject(ActivePACTConfig, Formatting.Indented);
            File.WriteAllText(fullpath, json);
        }

        public static void ImportHighPerformance(string fullpath)
        {
            BackupConfig();
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

            SaveConfig();
        }

        public static void ExportHighPerformance(string fullpath)
        {
            File.WriteAllLines(fullpath, ActivePACTConfig.GetHighPerformanceProcesses());
        }

        public static void ImportBlacklist(string fullpath)
        {
            BackupConfig();

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

            SaveConfig();
        }

        public static void ExportBlacklist(string fullpath)
        {
            File.WriteAllLines(fullpath, ActivePACTConfig.GetBlacklistedProcesses());
        }

        public static void ClearHighPerformance()
        {
            BackupConfig();
            var list = GetHighPerformanceProcesses();
            ActivePACTConfig.ClearHighPerformanceProcessList();
            SaveConfig();
        }

        public static void ClearCustoms()
        {
            BackupConfig();
            var list = GetCustomProcesses();
            ActivePACTConfig.ClearCustomPerformanceProcessList();
            SaveConfig();
        }

        public static void ClearBlackList()
        {
            BackupConfig();
            var list = GetBlacklistedProcesses();
            ActivePACTConfig.ClearBlacklist();
            SaveConfig();
        }

        public static void ResetConfig()
        {
            BackupConfig();
            ActivePACTConfig = new PACTConfig();
            SaveConfig();
        }




        public static IReadOnlyList<string> GetAllRunningProcesses()
        {
            // Todo: This is cursed.
            // Needs to be re-written so that it can return exe names
            return Process.GetProcesses()
                .Select(x => x.ProcessName)
                .OrderBy(x => x)
                .ToList() as IReadOnlyList<string>;
        }

        public static IReadOnlyList<string> GetProtectedProcesses()
        {
            return PACTProcessOverwatch.ProtectedProcesses.Select(x => x.ProcessName).OrderBy(x => x).ToList() as IReadOnlyList<string>;
        }

        public static IReadOnlyList<string> GetNormalPerformanceProcesses()
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

        public static IReadOnlyList<string> GetHighPerformanceProcesses()
        {
            return PACTProcessOverwatch.Config.GetHighPerformanceProcesses().OrderBy(x => x).ToList();
        }

        public static IReadOnlyList<string> GetCustomProcesses()
        {
            return PACTProcessOverwatch.Config.GetCustomPerformanceProcessConfigs().OrderBy(x => x).ToList();
        }

        public static IReadOnlyList<string> GetBlacklistedProcesses()
        {
            return PACTProcessOverwatch.Config.GetBlacklistedProcesses().OrderBy(x => x).ToList();
        }

        public static IReadOnlyList<int> GetHighPerformanceCores()
        {
            return PACTProcessOverwatch.Config.HighPerformanceProcessConfig.CoreList;
        }
        public static IReadOnlyList<int> GetNormalPerformanceCores()
        {
            return PACTProcessOverwatch.Config.DefaultPerformanceProcessConfig.CoreList;
        }




        public static void AddToBlacklist(string name)
        {
            ActivePACTConfig.AddToBlacklist(name);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void AddToHighPerformance(string name)
        {
            ActivePACTConfig.AddOrUpdate(name);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void AddToCustomPriority(string name, ProcessConfig conf)
        {
            ActivePACTConfig.AddOrUpdate(name, conf);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void ClearProcess(string name)
        {
            ActivePACTConfig.ClearProcessConfig(name);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void UpdateHighPerformanceProcessConfig(ProcessConfig conf)
        {
            ActivePACTConfig.HighPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void UpdateDefaultPriorityProcessConfig(ProcessConfig conf)
        {
            ActivePACTConfig.DefaultPerformanceProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }


    }
}
