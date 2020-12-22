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
            ActivePACTConfig = ReadConfig();
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

        public static PACTConfig ReadConfig()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "config.json";

            if (File.Exists(configPath))
            {
                string json = File.ReadAllText(configPath);
                PACTConfig tconf = JsonConvert.DeserializeObject<PACTConfig>(json);
                return tconf;
            }

            return new PACTConfig();
        }

        public static void SaveConfig()
        {
            string json = JsonConvert.SerializeObject(ActivePACTConfig, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);
        }

        public static PACTConfig ImportConfig(string fullpath)
        {
            if (File.Exists(fullpath))
            {
                string json = File.ReadAllText(fullpath);
                PACTConfig tconf = JsonConvert.DeserializeObject<PACTConfig>(json);
                return tconf;
            }
            else
            {
                throw new FileLoadException("Specified Config File Does not exist!");
            }
        }

        public static void ExportConfig(string fullpath)
        {
            string json = JsonConvert.SerializeObject(ActivePACTConfig, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory;
            File.WriteAllText(fullpath, json);
        }

        public static IReadOnlyList<string> GetRunningProcesses()
        {
            // Todo: This is cursed.
            // Needs to be re-written so that it can return exe names
            return Process.GetProcesses()
                .Select(x => x.ProcessName)
                .OrderBy(x => x)
                .ToList() as IReadOnlyList<string>;
        }

        public static IReadOnlyList<string> GetHighPriorityProcesses()
        {
            return ActivePACTConfig.HighPriorityProcessList.OrderBy(x => x).ToList() as IReadOnlyList<string>;
        }

        public static IReadOnlyList<string> GetCustomProcesses()
        {
            return ActivePACTConfig.CustomPriorityProcessList.Keys.OrderBy(x => x).ToList() as IReadOnlyList<string>;
        }

        public static IReadOnlyList<string> GetProtectedProcesses()
        {
            return PACTProcessOverwatch.ProtectedProcesses.Select(x => x.ProcessName).OrderBy(x => x).ToList() as IReadOnlyList<string>;
        }

        public static void AddToHighPriority(string name)
        {
            ActivePACTConfig.AddToHighPriority(name);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void AddToCustomPriority(string name, ProcessConfig conf)
        {
            ActivePACTConfig.AddToCustomPriority(name, conf);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void Clear(string name)
        {
            ActivePACTConfig.Clear(name);
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void UpdateHighPriorityProcessConfig(ProcessConfig conf)
        {
            ActivePACTConfig.HighPriorityProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void UpdateDefaultPriorityProcessConfig(ProcessConfig conf)
        {
            ActivePACTConfig.DefaultPriorityProcessConfig = conf;
            PACTProcessOverwatch.RunScan(true);
            SaveConfig();
        }

        public static void UpdateCustomPriorityProcessConfig(string name, ProcessConfig conf)
        {
            if (ActivePACTConfig.CustomPriorityProcessList.ContainsKey(name))
            {
                ActivePACTConfig.CustomPriorityProcessList[name] = conf;
                PACTProcessOverwatch.RunScan(true);
                SaveConfig();
            }
        }
    }
}
